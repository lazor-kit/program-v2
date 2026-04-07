---
# LazorKit – Protocol Architecture
---

## 1. Introduction

LazorKit is a next-generation Solana smart wallet, architected for maximum performance, account abstraction, and strong native security guarantees—even for modern authentication (WebAuthn/Passkey). It leverages zero-copy, deterministic Program-Derived Address (PDA) storage and flexible multi-role authorities for seamless dApp, automation, and end-user scenarios.

## 2. Design Highlights

- **Zero-Copy Layout:** All state is mapped with `NoPadding` and accessed directly via byte interpretation—no serialization overhead, always memory safe, ready for high-frequency, low-rent operations.
- **Full PDA Separation:** Every distinct wallet/authority/session/config/treasury is a deterministic, unique PDA. This gives unlimited key management and instant admin revocation without oversized accounts.
- **Role-Based RBAC**: Owner, Admin, and Spender roles are strictly separated (at the PDA/data level), with rigorous upgrade/prune logic tied to owner actions.
- **Replay & Binding Protection**: No signature or authority can be reused or replayed elsewhere (payload is always context-bound, includes hashes of pubkey/account/intent, and relies on explicit slot-height nonce logic for secp256r1).
- **Sharded Fee Treasury**: Revenue goes to one of many rent-exempt treasury shards, using round-robin or modular pubkey sharding; maximizes Solana parallelism, avoids write locks.
- **Versioning & Upgradability:** Every account struct is versioned for safe future program upgrades (with clear path for migration or expansion).

## 3. PDA + Account Structure

### Discriminators
All accounts are prefixed with a u8 discriminator (see enum below); all instruction flows enforce this, preventing spoof/collision.

```
#[repr(u8)]
pub enum AccountDiscriminator {
    Wallet = 1,
    Authority = 2,
    Session = 3,
    Config = 4,
}
```

### ConfigAccount
```
pub struct ConfigAccount {
  pub discriminator: u8, // must be 4
  pub bump: u8,
  pub version: u8,
  pub num_shards: u8,  // # treasury shards
  pub _padding: [u8; 4],
  pub admin: Pubkey,
  pub wallet_fee: u64,
  pub action_fee: u64,
}
```

### AuthorityAccountHeader
```
pub struct AuthorityAccountHeader {
  pub discriminator: u8, // 2
  pub authority_type: u8, // 0 = Ed25519, 1 = Secp256r1
  pub role: u8, // 0 = Owner, 1 = Admin, 2 = Spender
  pub bump: u8, pub version: u8,
  pub _padding: [u8; 3], pub counter: u64, pub wallet: Pubkey,
}
```

### SessionAccount
```
pub struct SessionAccount {
  pub discriminator: u8, pub bump: u8, pub version: u8, pub _padding: [u8; 5],
  pub wallet: Pubkey, pub session_key: Pubkey, pub expires_at: u64,
}
```

### WalletAccount
```
pub struct WalletAccount {
  pub discriminator: u8, pub bump: u8, pub version: u8, pub _padding: [u8; 5]
}
```

----

### PDA Derivation Table

| Account         | PDA Derivation Path                                       | Notes                                   |
|-----------------|----------------------------------------------------------|-----------------------------------------|
| Config PDA      | `["config"]`                                            | Protocol config, admin, shard count     |
| Treasury Shard  | `["treasury", u8_shard_id]`                             | Receives protocol fees                  |
| Wallet PDA      | `["wallet", user_seed]`                                 | User's main wallet anchor (addressable) |
| Vault PDA       | `["vault", wallet_pubkey]`                              | Holds wallet assets, owned by program   |
| Authority PDA   | `["authority", wallet, hash(id_bytes)]`                 | For each Authority (Owner/Admin/Spend)  |
| Session PDA     | `["session", wallet, session_pubkey]`                   | For ephemeral session authority         |

----

## 4. Instruction Workflow

**CreateWallet**
- Collects `wallet_fee` from payer to correct Treasury Shard
- Initializes Config (first), Wallet, Vault, and Owner Authority
- Assigns version – always checks for existing wallet

**Execute**
- Processes a compressed list of instructions (batched via CompactInstructions)
- Authenticates authority as:
    - Ed25519 (native Solana sig)
    - Secp256r1 (WebAuthn/Passkey, using SlotHashes sysvar for liveness proof)
    - Session (if session PDA is valid/not expired)
- Charges `action_fee` to payer routed to Treasury Shard
- Validates all discriminators, recalculates all seeds for PDAs
    - Strict binding: hash(pubkeys...) included in the approval payload

**Add/RemoveAuthority, UpdateRole**
- Owner/Admin may add authorities of any type
- Each authority is strictly attached to [wallet, id_hash]
- Roles can be upgraded or pruned; old authorities can be deactivated instantly

**CreateSession**
- Only Owner/Admin can create; sets future slot expiry; all permissions as Spender

**CloseSession**
- If expired: Protocol admin can close (recover rent/lamports)
- If active: Only wallet Owner or Admin can close

**CloseWallet**
- Full destroy, sweeps all wallet/vault lamports to destination
- Only Owner (role 0) may close wallet; all other authorities orphaned post-call

**UpdateConfig/InitTreasuryShard/SweepTreasury**
- Protocol admin can reconfigure protocol fees/treasury logic
- All protocol-level actions recalculate all seeds and enforce discriminator/owner match

----

## 5. Security Cornerstones

- **Discriminator Enforcement:** No account is interpreted without the correct type/material
- **Stack Depth Guarding:** All authentication flows enforce Solana stack height to prevent malicious cross-program invocations (CPI)
- **SlotHashes Nonce / Proof of Liveness:** For passkey authorities, all signoffs must target a recent Solana Slot (+150 slots), no on-chain counter required
- **Account/Instruction Binding:** No signature can be swapped; each critical payload is hashed/bound to the unique account pubkeys it targets (prevents cross-wallet replays)
- **Strict Reentrancy Protection:** All instruction flows have tight CPI limits, must be called directly by a signer or admin
- **Version-aware State:** Every on-chain struct includes a version field for safe upgrades

----
## 6. Client SDK Approach

- **Solita-based TypeScript SDK:** Autogenerated bindings (solita-client/) directly mirror Rust's NoPadding layout. All buffers/manual accounts constructed to match, including explicit offset mapping (avoids Beet/prefix issues).
- **High-level API (`LazorClient`):** Expose ergonomic calls for dApps/frontends, with automatic PDA resolution, pointer-safe transaction building, and Passkey ↔️ on-chain mapping logic.
- **Compression/Batching Ready:** SDK enables packing multiple high-level instructions into a single compressed Solana transaction for peak efficiency.

----
## 7. Source Layout

```
program/src/
├── auth/            # Ed25519, Secp256r1, and Passkey verification
├── processor/       # Per-instruction handler (wallet, authority, session, treasury, etc.)
├── state/           # All account definitions/discriminators
├── utils.rs, compact.rs # Helper / compression / fee code
├── lib.rs           # Program entrypoint + router
```

- `sdk/solita-client/` – TypeScript SDK tooling and runtime
- `tests-v1-rpc/` – E2E test suite; simulates real dApp usage and security regression

----
## 8. Upgrade Path and Extensibility

- Each on-chain struct is explicitly versioned and padded for smooth migration
- All PDA derivations withstand future expansion (admin can increase # treasury shards; new roles can be mapped; session/authority logic is modular)
- Interop: Solita SDK allows cutover to any client/chain that understands buffer layout

---
