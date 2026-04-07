# ⚡ LazorKit Smart Wallet (V2)

Modern, high-performance smart wallet protocol for Solana featuring multi-role RBAC (owner/admin/spender), extensible multi-protocol authentication (Passkey, Ed25519), stateless replay protection, and highly modular, scalable PDA-based storage. Built from the ground up for speed, security, and seamless account abstraction.

---

## 🚀 Key Features

- **Multi-Auth Protocols**: Support for both classic Ed25519 wallets and modern Passkey (WebAuthn/Secp256r1, Apple Secure Enclave), fully verified on-chain.
- **Dynamic Role-Based Access Control (RBAC):** Each authority is a uniquely derived PDA storing its role and type. Unlimited authorities, mapped cleanly by role: Owner (full), Admin (session mgmt), Spender (only execute). Strict role pruning & upgrades built-in.
- **Ephemeral Session Keys:** Issue temporary, slot-limited session sub-keys (with absolute expiry on Solana Slot) for programmable sessions and automation.
- **Treasury Sharding:** Protocol fees smartly distributed across `N` immutable treasury-shard PDAs for massively increased throughput and to avoid Solana's write-lock contention.
- **Deposit/Withdraw/Close:** Native destructibility—wallets and sessions can be safely closed, draining all SOL (including rent) back to the user, with full, secure authority checks.
- **Zero-Copy State, NoPadding:** Uses `pinocchio` for ultra-efficient raw-bytes interpretations, avoiding Borsh/Serde in all persistent state (drastically lowers CU cost and rent).
- **Full Security/Replay Protection:** All core Solana reentrancy/binding vectors covered: discriminator enforcement, explicit stack height guards, account/pubkey binding via hash, CPI replay guarded for cross-program safety, strict seed/path validation.
- **SDK & Compression:** TypeScript SDK with direct mapping to raw state layout (no prefix/padding mismatches!), plus transaction compression for extreme transaction packing (multiple calls, one TX).

---

## 🏗️ On-chain Account Types

| Account         | Seed(s) Format                                                | Purpose                                                                 |
|-----------------|--------------------------------------------------------------|-------------------------------------------------------------------------|
| Config PDA      | `["config"]`                                                | Global settings: admin pubkey, protocol fees, # treasury shards, version |
| Treasury Shard  | `["treasury", shard_id]`                                   | Sharded rent-exempt lamports storage, receives protocol fees             |
| Wallet PDA      | `["wallet", user_seed]`                                    | Main anchor for a user wallet, supports upgrades/versioning              |
| Vault PDA       | `["vault", wallet_pda]`                                    | Holds SOL/SPL owned by wallet (only signed by wallet PDA)                |
| Authority PDA   | `["authority", wallet, id_hash]`                           | Authority/role for wallet (Owner/Admin/Spender), can be Ed25519 or P256  |
| Session PDA     | `["session", wallet, session_pubkey]`                      | Temporary authority, expires after given slot, can be Ed25519            |

---

## 📋 State / Structs

All persistent accounts use the `NoPadding` layout and versioned headers. Example core account headers (see `program/src/state/*.rs`):

**ConfigAccount (global):**
```
pub struct ConfigAccount {
  pub discriminator: u8, // 4
  pub bump: u8,
  pub version: u8,
  pub num_shards: u8,
  pub _padding: [u8; 4],
  pub admin: Pubkey,
  pub wallet_fee: u64,
  pub action_fee: u64
}
```
**AuthorityAccountHeader:** (Ed25519 or Secp256r1)
```
pub struct AuthorityAccountHeader {
    pub discriminator: u8, // 2
    pub authority_type: u8, // 0=Ed25519, 1=Secp256r1
    pub role: u8, // 0=Owner, 1=Admin, 2=Spender
    pub bump: u8,
    pub version: u8,
    pub _padding: [u8; 3],
    pub counter: u64,
    pub wallet: Pubkey,
}
```
**SessionAccount:**
```
pub struct SessionAccount {
    pub discriminator: u8, // 3
    pub bump: u8,
    pub version: u8,
    pub _padding: [u8; 5],
    pub wallet: Pubkey,
    pub session_key: Pubkey,
    pub expires_at: u64,
}
```
**WalletAccount:**
```
pub struct WalletAccount {
    pub discriminator: u8, // 1
    pub bump: u8, pub version: u8, pub _padding: [u8; 5],
}
```

---

## 📝 Core Instruction Flow

- **CreateWallet**: Initializes Config, Wallet, Vault, and Owner-Authority PDA. Fee routed to correct Treasury Shard. No pre-fund DoS possible.
- **Execute**: Authenticates via Ed25519, Secp256r1, or Session PDA. Fee auto-charged to payer, routed by sharding. Strict discriminator & owner checks. Batch multiple actions using compacted instructions.
- **Add/RemoveAuthority / ChangeRole**: Owner and Admins may add, remove or rebind authorities, upgrading or pruning them.
- **CreateSession**: Only Owner or Admin can issue. Derives a session PDA with corresponding authority, slot expiry and version.
- **CloseSession**: Anyone (protocol Admin for expired, Owner/Admin for active+expired) can close and reclaim rent from a session.
- **CloseWallet**: Only Owner may destroy; all SOL in wallet+vault sent atomically to destination; permanent, secure cleanup. All authorities/sessions orphaned.
- **Protocol Fees/Config**: Protocol Admin can update fees at any time by updating Config PDA. Treasury Shards subdivide by hash(payer pubkey) % num_shards.
- **Init/Sweep TreasuryShard**: Admin can initialize and consolidate multiple treasury shards for optimal rent/fund flows.

---

## 🔐 Security & Protections

- **Discriminators Required:** All account loads must match the correct discriminator and layout.
- **Seed Enforcement:** All derived addresses are re-calculated and checked; spoofed config/wallet/authority is never possible.
- **Role Pruning:** Only protocol admin may reduce number of shards; authorities strictly match wallet+id.
- **Payload Binding:** All signature payloads are bound to concrete target accounts (destination when closing, self for session/exec). Execution hashes pubkeys for extra safety.
- **Reentrancy/Stack Checks:** All auth flows include stack height+slot introspection to block malicious CPIs.
- **SlotHashes Nonce:** For Passkey/Secp256r1: Each approval is valid strictly within +150 slots (approx. 75 seconds), leveraging on-chain sysvars & clientData reconstruction.
- **Rent/Destruct:** All PDA drains zero balances with safe arithmetic. Orphaned or unclaimed rent can only ever be collected by real wallet owner/admin.
- **Versioning:** All accounts are versioned, allowing smooth future upgrade paths.

---

## 📦 Project Structure

- `program/src/` – Solana program core: 
  - `auth/` Multi-auth modules (Ed25519, Secp256r1, Passkey, ...)
  - `processor/` – All instructions (wallet, authority, session, config, exec, treasury, ...)
  - `state/` – Account structs, discriminators, versioning
  - `utils.rs`, `compact.rs` – Support logic (fee collection, account compression, slot checks)
- `sdk/solita-client/` – Comprehensive TypeScript SDK supporting direct PDA/call logic
  - `src/wrapper.ts`, `src/utils/` – `LazorClient`: full wrapped, ergonomic app API
- `tests-v1-rpc/` – End-to-end test suites (localnet simulation, cross-role/race/destroy/fee scenarios)

---

## 🛠 Usage

**Build**
```bash
cargo build-sbf
```

**Test**
```bash
cd tests-v1-rpc
./scripts/test-local.sh
```

---

## 📑 License

MIT
