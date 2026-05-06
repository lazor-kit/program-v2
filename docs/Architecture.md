# LazorKit Architecture

## 1. Overview

LazorKit is a high-performance smart wallet on Solana with passkey (WebAuthn) authentication, role-based access control, and session keys. Built with pinocchio for zero-copy serialization.

## 2. Core Principles

- **Zero-Copy**: pinocchio casts raw bytes to Rust structs, no Borsh.
- **NoPadding**: custom derive ensures memory safety and tight packing.
- **Separated Storage**: each authority gets its own PDA (unlimited per wallet, no resize).
- **Strict RBAC**: Owner (0), Admin (1), Spender (2).
- **CompactInstructions**: index-based instruction referencing for Execute.

## 3. Security Mechanisms

### Replay Protection

- **Secp256r1 (Primary: Odometer Counter)**: Program-controlled u32 counter per authority. Client submits `stored_counter + 1`. The WebAuthn hardware counter is intentionally NOT used -- synced passkeys (iCloud, Google) return unreliable values. Counter is committed only after successful signature verification.
- **Secp256r1 (Secondary: Clock-based Slot Freshness)**: Slot from auth_payload must be within 150 slots of `Clock::get()`. Provides freshness without stateful nonces or the SlotHashes sysvar.
- **Secp256r1 (CPI Protection)**: stack_height check prevents authentication via CPI.
- **Secp256r1 (Signature Binding)**: Challenge hash binds signature to specific instruction, payer, accounts, counter, and program_id.
- **Ed25519**: Standard Solana runtime signer verification. No counter needed.
- **Sessions**: Absolute slot-based expiry. Max duration ~30 days.

### Challenge Hash (Secp256r1)

```
SHA256(discriminator || auth_payload || signed_payload || slot || payer || counter || program_id)
```

7 elements, computed on-chain via `sol_sha256` syscall.

### WebAuthn Passkey Support

- Embeds raw `clientDataJSON` directly in the auth payload; the program parses the JSON on-chain (extracts `type` + `challenge` fields, validates `webauthn.get`, base64url-compares challenge against the recomputed digest).
- Verifies authenticatorData flags (User Presence / User Verification).
- Verifies `rpIdHash` against the precomputed digest stored on the authority account at registration (saves one `sol_sha256` syscall per Execute).
- Uses Secp256r1SigVerify precompile via sysvar introspection.
- Stores 33-byte compressed public keys (not 64-byte uncompressed).

## 4. Account Structure (PDAs)

### Discriminators

```rust
#[repr(u8)]
pub enum AccountDiscriminator {
    Wallet = 1,
    Authority = 2,
    Session = 3,
    DeferredExec = 4,
}
```

### A. WalletAccount (8 bytes)

Seeds: `["wallet", user_seed]`

```rust
#[repr(C, align(8))]
pub struct WalletAccount {
    pub discriminator: u8,   // 1 = Wallet
    pub bump: u8,
    pub version: u8,
    pub _padding: [u8; 5],
}
// Total: 8 bytes
```

### B. AuthorityAccountHeader (48 bytes) + Variable Data

Seeds: `["authority", wallet_pubkey, id_hash]`

```rust
#[repr(C, align(8))]
pub struct AuthorityAccountHeader {
    pub discriminator: u8,   // 2 = Authority
    pub authority_type: u8,  // 0=Ed25519, 1=Secp256r1
    pub role: u8,            // 0=Owner, 1=Admin, 2=Spender
    pub bump: u8,
    pub version: u8,
    pub _padding1: [u8; 3],
    pub counter: u32,        // Monotonic u32 odometer for Secp256r1 replay protection
    pub _padding2: [u8; 4],  // Alignment padding (wallet stays at offset 16)
    pub wallet: Pubkey,      // 32 bytes
}
// Header: 1+1+1+1+1+3+4+4+32 = 48 bytes (same size, wallet at same offset)
```

Variable data after header:

- **Ed25519**: `[pubkey: [u8; 32]]` -- total 80 bytes.
- **Secp256r1**: `[credential_id_hash: [u8; 32]] [compressed_pubkey: [u8; 33]] [rpIdHash: [u8; 32]]` -- total 145 bytes. The rpId is hashed once at creation and the digest stored on-chain so every subsequent `Execute` saves one `sol_sha256` syscall.

### C. SessionAccount (80-byte fixed header + optional action buffer)

Seeds: `["session", wallet_pubkey, session_key]`

```rust
#[repr(C, align(8))]
pub struct SessionAccount {
    pub discriminator: u8,   // 3 = Session
    pub bump: u8,
    pub version: u8,
    pub _padding: [u8; 5],
    pub wallet: Pubkey,      // 32 bytes
    pub session_key: Pubkey, // 32 bytes
    pub expires_at: u64,     // Absolute slot height
}
// Header: 1+1+1+5+32+32+8 = 80 bytes
```

Optional **actions buffer** appended after the 80-byte header (max 16 actions, ≤ 2048 bytes). Each action: `[type: u8][data_len: u16 LE][expires_at: u64 LE][data: N]`.

Action types (must match `state/action.rs::ActionType`):

| Discriminator | Type | Data |
|---|---|---|
| 1 | `SolLimit` | `remaining: u64` (lifetime SOL spending cap) |
| 2 | `SolRecurringLimit` | `limit: u64, spent: u64, window: u64, last_reset: u64` |
| 3 | `SolMaxPerTx` | `max: u64` (per-execute SOL ceiling) |
| 4 | `TokenLimit` | `mint: [u8;32], remaining: u64` |
| 5 | `TokenRecurringLimit` | `mint: [u8;32], limit, spent, window, last_reset` |
| 6 | `TokenMaxPerTx` | `mint: [u8;32], max: u64` |
| 10 | `ProgramWhitelist` (repeatable) | `program_id: [u8;32]` |
| 11 | `ProgramBlacklist` (repeatable) | `program_id: [u8;32]` |

Enforcement runs in `processor/execute_actions.rs`: pre-CPI program whitelist/blacklist checks + token-balance + token-authority snapshots; post-CPI delta computation, SOL/token cap enforcement with saturating arithmetic, recurring-window resets aligned to slot boundaries, and vault-invariant defenses against `System::Assign` / `SetAuthority` / `Approve` escapes (errors 3030–3032).

### D. DeferredExecAccount (176 bytes)

Seeds: `["deferred", wallet_pubkey, authority_pubkey, counter_le(4)]`

```rust
#[repr(C, align(8))]
pub struct DeferredExecAccount {
    pub discriminator: u8,           // 4 = DeferredExec
    pub version: u8,
    pub bump: u8,
    pub _padding: [u8; 5],
    pub instructions_hash: [u8; 32], // SHA256 of serialized compact instructions
    pub accounts_hash: [u8; 32],     // SHA256 of all account pubkeys referenced
    pub wallet: Pubkey,              // 32 bytes
    pub authority: Pubkey,           // 32 bytes — the authority that authorized
    pub payer: Pubkey,               // 32 bytes — receives rent refund on close
    pub expires_at: u64,             // Absolute slot at which this expires
}
// Total: 1+1+1+5+32+32+32+32+32+8 = 176 bytes
```

Temporary account created during `Authorize` (tx1) and closed during `ExecuteDeferred` (tx2). Uses the authority's odometer counter as a seed nonce, ensuring unique PDAs per authorization. Expired accounts can be reclaimed via `ReclaimDeferred`.

### E. Vault PDA

Seeds: `["vault", wallet_pubkey]`

No data allocated. Holds SOL. Program signs for it via PDA seeds during Execute.

## Parallel Execution

A key design property: **different authorities on the same wallet can execute transactions in parallel** on Solana's runtime.

### Why it works

During Execute, the only account written to is the **authority PDA** (odometer counter increment). The wallet PDA and vault PDA are read-only:

| Account | Access | Shared across authorities? |
|---|---|---|
| Authority PDA | **Writable** (counter++) | No -- each authority has its own PDA |
| Wallet PDA | Read-only | Yes, but no write lock |
| Vault PDA | Signer-only (CPI) | Yes, but no write lock |

Since each authority is a separate PDA, Solana's scheduler sees no writable overlap and runs them concurrently.

### Parallelism matrix

| Scenario | Parallel? | Reason |
|---|---|---|
| Authority A + Authority B (same wallet) | Yes | Different writable PDAs |
| Session key + Secp256r1 authority (same wallet) | Yes | Different writable PDAs |
| Same authority, 2 transactions | No | Same writable PDA + counter conflict |
| Authority A (wallet 1) + Authority B (wallet 2) | Yes | Entirely separate accounts |

### Design implication

This enables high-throughput wallets where multiple authorized parties (e.g., an admin managing permissions while a spender sends payments, or multiple session keys operating concurrently) never block each other. The per-authority odometer counter provides replay protection without creating a shared bottleneck.

## 5. Instructions (10 total)

### CreateWallet (discriminator: 0)

- Creates Wallet PDA, Vault PDA (derived only), and first Authority PDA.
- Transfer-Allocate-Assign pattern to prevent pre-funding DoS.
- Accounts: payer, wallet, vault, authority, system_program, rent_sysvar.

### AddAuthority (discriminator: 1)

- Creates new Authority PDA.
- Requires Admin or Owner authentication.
- Owner can add any role; Admin can only add Spender.
- Accounts: payer, wallet, admin_authority, new_authority, system_program, rent_sysvar [+ sysvar_instructions for Secp256r1].

### RemoveAuthority (discriminator: 2)

- Closes Authority PDA, refunds rent to specified destination.
- Prevents self-removal and owner removal (ownership must be transferred).
- Accounts: payer, wallet, admin_authority, target_authority, refund_destination.

### TransferOwnership (discriminator: 3)

- Atomically closes old owner and creates new owner.
- Accounts: payer, wallet, current_owner, new_owner_authority, system_program, rent_sysvar.

### Execute (discriminator: 4)

- Executes CompactInstructions via CPI with vault PDA signing.
- Supports 3 auth modes: Ed25519 signer, Secp256r1 (with precompile), Session key.
- Self-reentrancy protection: rejects CPI back into this program.
- Accounts: payer, wallet, authority/session, vault, [remaining accounts...].

### CreateSession (discriminator: 5)

- Creates ephemeral Session PDA with slot-based expiry.
- Requires Admin or Owner.
- Validates expires_at: must be in future, max ~30 days.
- Accounts: payer, wallet, authorizer, session, system_program, rent_sysvar.

### Authorize (discriminator: 6) — Deferred Execution TX1

- Creates a DeferredExec PDA storing pre-authorized instruction/account hashes.
- Only Secp256r1 Owner/Admin can authorize (not Ed25519, not Spender).
- Signed payload: `instructions_hash || accounts_hash || expiry_offset` (66 bytes).
- Expiry offset bounded to 10-9,000 slots (~4 seconds to ~1 hour).
- Uses the authority's odometer counter (post-increment) as PDA seed nonce.
- Instruction data: `[instructions_hash(32)][accounts_hash(32)][expiry_offset(2)][auth_payload(variable)]`.
- Accounts: payer, wallet, authority, deferred_exec, system_program, rent_sysvar, sysvar_instructions.

### ExecuteDeferred (discriminator: 7) — Deferred Execution TX2

- Verifies compact instructions against stored hashes, executes via CPI with vault PDA signing.
- Closes the DeferredExec account before CPI (close-before-execute pattern).
- Verifies both instructions_hash and accounts_hash match stored values.
- Checks expiry (must not be past `expires_at` slot).
- Refunds rent to the original payer (stored in DeferredExec).
- Self-reentrancy protection: rejects CPI back into this program.
- Instruction data: `[compact_instructions(variable)]`.
- Accounts: payer, wallet, vault, deferred_exec, refund_destination, [remaining accounts...].

### ReclaimDeferred (discriminator: 8)

- Closes an expired DeferredExec account and refunds rent to the original payer.
- Only the original payer (stored in `deferred.payer`) can reclaim.
- Can only be called after `expires_at` has passed.
- No instruction data (discriminator only).
- Accounts: payer, deferred_exec, refund_destination.

### RevokeSession (discriminator: 9)

- Closes a session account early (before expiry), refunding rent.
- Only Owner or Admin can revoke (Spender cannot).
- Session can be revoked regardless of whether it is expired or active.
- Signature bound to specific session PDA + refund destination (prevents replay).
- Accounts: payer, wallet, admin_authority, session, refund_destination [+ auth_extra].

## 6. CompactInstructions Format

Binary format for packing multiple instructions into Execute:

```
[num_instructions: u8]
For each instruction:
  [program_id_index: u8]      // Index into transaction accounts
  [num_accounts: u8]
  [account_indexes: u8[]]     // Indexes into transaction accounts
  [data_len: u16 LE]
  [instruction_data: u8[]]
```

Overhead per instruction: 4 bytes + num_accounts. Replaces 32-byte pubkeys with 1-byte indexes.

### Accounts Hash (Anti-Reordering)

For Secp256r1 Execute, the signed payload includes a SHA256 hash of all account pubkeys referenced by the compact instructions. This prevents account reordering attacks where an attacker could swap recipient addresses while keeping the signature valid.

## 7. Deferred Execution

2-transaction flow for payloads exceeding the ~574 bytes available in a single Secp256r1 Execute transaction (e.g., Jupiter swaps with complex routing).

### Flow

1. **TX1 (Authorize)**: Client computes `instructions_hash = SHA256(packed_compact_instructions)` and `accounts_hash = SHA256(all_referenced_pubkeys)`. These hashes are signed via Secp256r1 and stored in a DeferredExec PDA. The authority's odometer counter is incremented.
2. **TX2 (ExecuteDeferred)**: Any signer submits the full compact instructions. The program verifies both hashes match, checks expiry, closes the DeferredExec account, and executes via CPI with vault signing.

### Capacity

| Path | Inner Ix Capacity | Total CU | Tx Fee |
|---|---|---|---|
| Immediate Execute | ~574 bytes | 9,441 | 0.000005 SOL |
| Deferred (2 txs) | ~1,100 bytes (1.9x) | 15,613 | 0.00001 SOL |

### Security Properties

- **Hash binding**: Both instruction content and account ordering are hash-verified.
- **Replay protection**: Odometer counter used as PDA seed nonce — each authorization gets a unique PDA.
- **Expiry**: 10-9,000 slot window (~4s to ~1h). Prevents stale authorizations.
- **Role gating**: Only Secp256r1 Owner/Admin can authorize.
- **Close-before-CPI**: DeferredExec account is closed before CPI execution, avoiding stale-pointer issues with `invoke_signed_unchecked`. Transaction reverts atomically if any CPI fails.
- **Rent recovery**: `ReclaimDeferred` allows original payer to reclaim rent from expired, unexecuted authorizations.

## 8. Auth Payload Layout (Secp256r1)

```
[slot: u64 LE]              // 8 bytes  -- Clock-based slot freshness
[counter: u32 LE]           // 4 bytes  -- odometer value (stored + 1)
[sysvar_ix_index: u8]       // 1 byte   -- index of sysvar_instructions in accounts
[type_and_flags: u8]        // 1 byte   -- WebAuthn type + flags
[authenticator_data: u8[]]  // M bytes  -- WebAuthn authenticator data (min 37 bytes)
```

Compared to the previous layout, 3 optimizations reduce the per-transaction payload:
- **Counter**: u64 -> u32 (saves 4 bytes; 4 billion ops per authority is sufficient)
- **SlotHashes index**: removed (slot freshness via `Clock::get()` instead of sysvar lookup)
- **rpId**: stored on the authority account at creation, not sent per-tx (saves ~12 bytes)

## 9. Project Structure

```
program/
  src/
    auth/
      ed25519.rs              Native signer verification
      secp256r1/
        mod.rs                Passkey authenticator with odometer + Clock-based slot check
        introspection.rs      Precompile instruction verification
        webauthn.rs           Raw clientDataJSON validation + AuthDataParser
      traits.rs               Authenticator trait
    processor/
      create_wallet.rs
      manage_authority.rs     AddAuthority + RemoveAuthority
      execute.rs              CompactInstruction execution (immediate)
      execute_actions.rs      Pre/post action enforcement engine (token snapshots, vault invariants)
      authorize.rs            Deferred execution TX1 (creates DeferredExec PDA)
      execute_deferred.rs     Deferred execution TX2 (verifies + executes)
      reclaim_deferred.rs     Closes expired DeferredExec accounts
      create_session.rs       Session creation with optional action buffer
      revoke_session.rs       Owner/Admin can close session early, refund rent
      transfer_ownership.rs
    state/
      wallet.rs               WalletAccount (8 bytes)
      authority.rs            AuthorityAccountHeader (48 bytes)
      session.rs              SessionAccount (80-byte header + optional actions buffer)
      deferred.rs             DeferredExecAccount (176 bytes)
      action.rs               Session action types + parser + validator (8 types, 11-byte header)
    compact.rs                CompactInstruction serialization (owned + zero-copy ref variants)
    utils.rs                  PDA initialization, stack_height check
    error.rs                  AuthError enum (3001-3032)
    entrypoint.rs             Instruction routing (disc 0–9)
tests-sdk/                    Integration + security tests (vitest, 65 tests)
```

The TypeScript SDK lives outside this repo: `@lazorkit/sdk-legacy` (in
sibling `lazorkit-protocol` repo at `sdk/sdk-legacy/`). Same SDK works
against this build (foundation, no fee) and the commercial build —
probes ProtocolConfig at runtime and conditionally appends fee accounts.
