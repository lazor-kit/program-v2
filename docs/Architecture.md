# LazorKit Architecture

## 1. Overview
LazorKit is a high-performance, secure smart wallet on Solana designed for distinct Authority storage and modern authentication (Passkeys). It uses `pinocchio` for efficient zero-copy state management and optimized syscalls.

## 2. Core Principles
*   **Zero-Copy Executions**: We strictly use `pinocchio` to cast raw bytes into Rust structs. This bypasses Borsh serialization overhead for maximum performance.
    *   *Requirement*: All state structs must implement `NoPadding` (via `#[derive(NoPadding)]`) to ensure memory safety and perfect packing.
*   **Separated Storage (PDAs)**: Uses a deterministic PDA for *each* Authority.
    *   *Benefit*: Unlimited distinct authorities per wallet without resizing costs.
*   **Strict RBAC**: Hardcoded Permission Roles (Owner, Admin, Spender) for predictability.
*   **Transaction Compression**: Uses `CompactInstructions` (Index-based referencing) to fit complex multi-call payloads into standard Solana transaction limits (~1232 bytes).

## 3. Security Mechanisms

### Replay Protection
*   **Ed25519**: Relies on standard Solana runtime signature verification.
*   **Secp256r1 (WebAuthn)**:
    *   **SlotHashes Nonce**: Uses the `SlotHashes` sysvar to verify that the signed payload references a recent slot (within 150 slots). This acts as a "Proof of Liveness" without requiring on-chain nonces.
    *   **CPI Protection**: Explicit `stack_height` check prevents the authentication instruction from being called via CPI, defending against cross-program replay attacks.
*   **Session Keys**:
    *   **Slot-Based Expiry**: Sessions are valid until a specific absolute slot height `expires_at` (u64).

### WebAuthn "Passkey" Support
*   **Modules**: `program/src/auth/secp256r1`
*   **Mechanism**:
    *   Reconstructs `clientDataJson` on-chain from `challenge` (nonce) and `origin`.
    *   Verifies `authenticatorData` flags (User Presence/User Verification).
    *   Uses `Secp256r1SigVerify` precompile via Sysvar Introspection.
    *   Uses `Secp256r1SigVerify` precompile via Sysvar Introspection.
    *   *Abstraction*: Implements the `Authority` trait for unified verification logic.
## 4. Account Structure (PDAs)

### Discriminators
```rust
#[repr(u8)]
pub enum AccountDiscriminator {
    Wallet = 1,
    Authority = 2,
    Session = 3,
}
```

### A. Authority Account
Represents a single authorized user.
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
    *   **Type 1 (Secp256r1)** adds: `[ u32 padding ] + [ credential_id_hash (32) ] + [ pubkey (64) ]`.

### B. Session Account (Ephemeral)
Temporary sub-key for automated agents.
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
*   Initializes `WalletAccount`, `Vault`, and the first `Authority` (Owner).
*   Derives `AuthorityPDA` using `role=0`.

### `Execute`
*   **Authentication**:
    *   **Ed25519**: Runtime signer check.
    *   **Secp256r1**:
        1.  Reconstruct `clientDataJson` using `current_slot` (must match signed challenge).
        2.  Verify `SlotHashes` history to ensure `current_slot` is within 150 blocks of now.
        3.  Verify Signature against `sha256(clientDataJson + authData)`.
    *   **Session**:
        1.  Verify `session.wallet == wallet`.
        2.  Verify `current_slot <= session.expires_at`.
        3.  Verify `session_key` is a signer.
*   **Decompression**: Expands `CompactInstructions` and calls `invoke_signed` with Vault seeds.

### `CreateSession`
*   **Auth**: Requires `Role::Admin` or `Role::Owner`.
*   **Expiry**: Sets `expires_at` to a future slot height (u64).

## 6. Project Structure
```
program/src/
├── auth/
│   ├── ed25519.rs       # Native signer checks
│   ├── secp256r1/
│   │   ├── mod.rs       # Main logic
│   │   ├── nonce.rs     # SlotHashes validation
│   │   ├── slothashes.rs# Sysvar parser
│   │   └── webauthn.rs  # ClientDataJSON utils
├── processor/           # Instruction handlers
├── state/               # Account definitions (NoPadding)
├── utils.rs             # Helper functions (stack_height)
└── lib.rs               # Entrypoint
```
