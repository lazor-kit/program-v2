# LazorKit Architecture

## 1. Overview
LazorKit is a high-performance, secure smart wallet on Solana designed for distinct Authority storage and modern authentication (Passkeys). It uses `pinocchio` for efficient zero-copy state management and optimized syscalls.

## 2. Core Principles
*   **Zero-Copy Executions**: We strictly use `pinocchio` to cast raw bytes into Rust structs. This bypasses Borsh serialization overhead for maximum performance.
    *   *Requirement*: All state structs must implement `NoPadding` (via `#[derive(NoPadding)]`) to ensure memory safety and perfect packing.
*   **Separated Storage (PDAs)**: Uses a deterministic PDA for *each* Authority.
    *   *Benefit*: Unlimited distinct authorities per wallet without resizing costs or hitting `10KiB` limits limits inside a single account.
*   **Strict RBAC**: Hardcoded Permission Roles (Owner, Admin, Spender) for predictability and secure delegation.
*   **Transaction Compression**: Uses `CompactInstructions` (Index-based referencing) to fit complex multi-call payloads into standard Solana transaction limits (~1232 bytes).

## 3. Security Mechanisms

### Replay Protection
*   **Ed25519**: Relies on standard Solana runtime signature verification.
*   **Secp256r1 (WebAuthn)**:
    *   **SlotHashes Nonce**: Uses the `SlotHashes` sysvar to verify that the signed payload references a recent slot (within 150 slots). This acts as a highly efficient "Proof of Liveness" without requiring stateful on-chain nonces or counters.
    *   **CPI Protection**: Explicit `stack_height` check prevents the authentication instruction from being called maliciously via CPI (`invoke`), defending against cross-program attack vectors.
    *   **Signature Binding**: The `auth_payload` ensures signatures are tightly bound to the specific target account and instruction data. For `Execute`, it dynamically computes a `sha256` hash of all provided `AccountInfo` pubkeys and binds the signature to it, preventing cross-wallet, cross-instruction, or account reordering attacks.
*   **Session Keys**:
    *   **Absolute Expiry**: Sessions are valid until a strict absolute slot height `expires_at` (u64).

### WebAuthn "Passkey" Support
*   **Modules**: `program/src/auth/secp256r1`
*   **Mechanism**:
    *   Reconstructs `clientDataJson` on-chain dynamically based on the current context (`challenge` and `origin`).
    *   Verifies `authenticatorData` flags (User Presence/User Verification).
    *   Uses `Secp256r1SigVerify` precompile via Sysvar Introspection (`load_current_index_param`).
    *   *Abstraction*: Implements the `Authority` trait for unified verification logic across both key types.

## 4. Account Structure (PDAs)

### Discriminators
```rust
#[repr(u8)]
pub enum AccountDiscriminator {
    Wallet = 1,
    Authority = 2,
    Session = 3,
    Config = 4,
}
```

### A. Config Account
Global protocol configuration holding action and wallet creation fees.
*   **Seeds**: `[b"config"]`
*   **Data Structure**: Stores `admin`, `wallet_fee`, `action_fee`, and `num_shards` count.

### B. Wallet Account
Anchor for the identity.
*   **Seeds**: `[b"wallet", user_seed]`

### C. Vault Account
The primary asset-holding PDA.
*   **Seeds**: `[b"vault", wallet_pubkey]`

### D. Treasury Shard
Receives protocol fees collected from user transactions. Derived via modular arithmetic on the payer's pubkey.
*   **Seeds**: `[b"treasury", shard_id]`
*   **Data Structure**: None. Raw lamports PDA to prevent serialization costs.

### E. Authority Account
Represents a single authorized user (Owner, Admin, or Spender). Multi-signature schemas allow deploying multiple Authority PDAs per Wallet.
*   **Seeds**: `[b"authority", wallet_pubkey, id_hash]`
*   **Data Structure**:
    ```rust
    #[repr(C)]
    #[derive(NoPadding)]
    pub struct AuthorityAccountHeader {
        pub discriminator: u8,
        pub authority_type: u8, // 0=Ed25519, 1=Secp256r1
        pub role: u8,           // 0=Owner, 1=Admin, 2=Spender
        pub bump: u8,
        pub wallet: Pubkey,
    }
    ```
    *   **Type 1 (Secp256r1)** adds padding to reach 8-byte alignment: `[ u32 padding ] + [ credential_id_hash (32) ] + [ pubkey (64) ]`.

### C. Session Account (Ephemeral)
Temporary sub-key for automated agents (like a session token).
*   **Seeds**: `[b"session", wallet_pubkey, session_key]`
*   **Data Structure**:
    ```rust
    #[repr(C, align(8))]
    #[derive(NoPadding)]
    pub struct SessionAccount {
        pub discriminator: u8,
        pub bump: u8,
        pub _padding: [u8; 6],
        pub wallet: Pubkey,
        pub session_key: Pubkey,
        pub expires_at: u64, // Absolute Slot Height
    }
    ```

## 5. Instruction Flow & Logic

### `CreateWallet`
*   **Fees**: Parses the global `Config PDA` to charge `wallet_fee` dynamically assigning the collected amount to a calculated `Treasury Shard`.
*   Initializes `WalletAccount`, `Vault`, and the first `Authority` (Owner).
*   Follows the Transfer-Allocate-Assign pattern to prevent pre-funding DoS attacks.

### `Execute`
*   **Fees**: Automatically parses the global `Config PDA` to collect `action_fee` to the calculated `Treasury Shard`.
*   **Authentication**:
    *   **Ed25519**: Standard Instruction Introspection.
    *   **Secp256r1**:
        1.  Verify `SlotHashes` history to ensure the signed `current_slot` is within 150 blocks of the exact current network slot.
        2.  Reconstruct `clientDataJson` using that `current_slot`.
        3.  Verify Signature against `sha256(clientDataJson + authData)`.
    *   **Session**:
        1.  Verify `session.wallet == wallet`.
        2.  Verify `current_slot <= session.expires_at`.
        3.  Verify `session_key` is a valid Ed25519 signer on the transaction.
*   **Decompression**: Expands `CompactInstructions` and calls `invoke_signed_unchecked` with Vault seeds across all requested target programs.

### `CreateSession`
*   **Auth**: Requires `Role::Admin` or `Role::Owner`.
*   **Action**: Derives the correct Session PDA and sets `expires_at` to a future slot height (u64).

### `CloseSession`
*   **Auth**: Custom dual-mode authorization. If the session has expired, the protocol `admin` can close it. If it is active, the `Owner` or `Admin` of the Wallet PDA can close it.
*   **Action**: Refunds the session's rent exemption Lamports back to the instruction `payer`.

### `CloseWallet`
*   **Auth**: Only `Role::Owner` can execute this.
*   **Action**: Extremely destructive. Collects all lamports from the `wallet_pda` and `vault_pda` state and sweeps them strictly to the designated `destination` account securely, wiping local binary limits.

### Protocol Management
*   **InitializeConfig / UpdateConfig**: Establishes global parameters managed heavily by the deployment admin.
*   **InitTreasuryShard / SweepTreasury**: Admin instructions to sweep protocol revenue down to a single master vault while retaining mandatory 0-byte rent exemption balances in the shards to prevent permanent BPF runtime account closure exceptions.

## 6. Client SDK Abstraction (Solita)
The TypeScript SDK (`solita-client`) relies on a two-tier approach to interface cleanly with the `NoPadding` C-Structs output by Rust.
### A. `LazorInstructionBuilder` (Low-Level)
Because Solita commonly auto-injects a standardized `4-byte` length prefix to buffer types (via `@metaplex-foundation/beet`), the SDK completely bypasses the generated `createWallet`, `addAuthority`, and `transferOwnership` payload generators. It uses manually constructed `Buffer.alloc` maps reflecting EXACT byte offsets defined in Rust.
### B. `LazorClient` (High-Level Wrapper)
An ergonomic layer providing developers with automatically resolved PDAs, transaction compaction methods, and native Web Crypto API wrappers to seamlessly translate WebAuthn responses into pre-packaged Secp256r1 `Execution` Instructions.

## 7. Project Structure
```text
program/src/
├── auth/
│   ├── ed25519.rs       # Native signer verification
│   ├── secp256r1/
│   │   ├── mod.rs       # Passkey entrypoint
│   │   ├── nonce.rs     # SlotHashes verification logic
│   │   ├── slothashes.rs# Sysvar memory-parsing
│   │   └── webauthn.rs  # ClientDataJSON reconstruction
├── processor/           # Handlers
│   ├── initialize_config.rs / update_config.rs
│   ├── create_wallet.rs / create_session.rs
│   ├── execute.rs / transfer_ownership.rs
│   ├── init_treasury_shard.rs / sweep_treasury.rs
│   └── close_wallet.rs / close_session.rs
├── state/               # NoPadding definitions (Wallet, Authority, Session, Config)
├── utils.rs             # Protections (stack_height, dos_prevention), fee_collection logic
├── compact.rs           # Account-index compression tools
└── lib.rs               # Entrypoint & routing
```
