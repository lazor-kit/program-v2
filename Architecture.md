# LazorKit Architecture

## 1. Overview
LazorKit is a high-performance, secure smart wallet on Solana designed for distinct Authority storage and modern authentication (Passkeys). It uses `pinocchio` for efficient zero-copy state management.

## 2. Core Principles
*   **Zero-Copy Executions**: We strictly use `pinocchio` to cast raw bytes into Rust structs. This bypasses Borsh serialization overhead, significantly reducing Compute Unit (CU) usage.
    *   *Requirement*: All state structs must implement `NoPadding` to ensure memory safety.
*   **Separated Storage (PDAs)**: Unlike traditional wallets that store all authorities in one generic account (limiting scale), LazorKit uses a deterministic PDA for *each* Authority.
    *   *Benefit*: Unlimited distinct authorities per wallet. No resizing costs.
*   **Strict RBAC**: Hardcoded Permission Roles (Owner, Admin, Spender) replace complex dynamic policies for security and predictability.
*   **Transaction Compression**: Uses `CompactInstructions` (Index-based referencing) to fit complex multi-call payloads into standard Solana transaction limits (~1232 bytes).

## 3. Swig Wallet Technical Adoption
We reference `swig-wallet` for high-performance patterns but simplify the governance layer.

| Feature | Swig Implementation | LazorKit Adoption Detail |
| :--- | :--- | :--- |
| **Zero-Copy Safety** | `#[derive(NoPadding)]` proc-macro. | **MANDATORY**. We will implement this macro to enforce `repr(C, align(8))` and assert no hidden padding bytes exist. |
| **Replay Protection** | `signature_odometer` (u32 monotonic counter). | **MANDATORY** for Secp256r1 (Passkeys). Since Passkeys sign a non-nonce challenge, we must track and increment an on-chain counter to prevent replay attacks. |
| **Passkey Parsing** | Huffman decoding + JSON reconstruction for WebAuthn. | **MANDATORY**. Real Passkeys (Apple/Google) wrap signatures in complex JSON. We must parse this on-chain to verify the correct challenge was signed. |
| **Compression** | `CompactInstructions` (u8 indexes vs 32-byte keys). | **MANDATORY**. Standard instructions are too large for extensive composition. We adopt the compact format to enable "Smart Wallet" composability. |
| **Assertions** | `sol_memcmp_` syscalls. | **ADOPT**. We will use optimized syscalls instead of Rust's `==` to save CUs on pubkey comparisons. |

## 4. Account Structure (PDAs)

### Discriminators
We use a centralized `u8` enum to distinguish account types.
```rust
#[repr(u8)]
pub enum AccountDiscriminator {
    Wallet = 1,
    Authority = 2,
    Session = 3,
}
```

### A. Wallet Account (Global State)
The root anchor for a wallet instance.
*   **Seeds**: `[b"wallet", user_seed]`
    *   `user_seed`: `&[u8]` (Max 32 bytes). User-provided salt to differentiate wallets.
*   **Space**: 16 bytes.
*   **Data Structure**:
    ```rust
    #[repr(C, align(8))]
    #[derive(NoPadding)]
    pub struct WalletAccount {
        pub discriminator: u8,  // 1 (AccountDiscriminator::Wallet)
        pub bump: u8,           // 1 byte
        pub _padding: [u8; 6],  // 6 bytes (Align to 8)
    }
    ```

### B. Authority Account (Member)
Represents a single authorized user (Key or Passkey).
*   **Seeds**: `[b"authority", wallet_pubkey, id]`
    *   `id`: `[u8; 32]`.
    *   **Ed25519**: `id = Public Key`.
    *   **Secp256r1**: `id = sha256(Credential Public Key)`.
*   **Space**: Variable (Header + Key Length).
    *   **Ed25519**: 72 + 32 = 104 bytes.
    *   **Secp256r1**: 72 + 33 = 105 bytes.
*   **Data Structure**:
*   **Structure**: `[ Header (36 bytes) ] + [ Type-Specific Data ]`

**1. Common Header (36 bytes)**
```rust
#[repr(C)]
#[derive(NoPadding, Debug, Clone, Copy)]
pub struct AuthorityAccountHeader {
    pub discriminator: u8,       // 1 (AccountDiscriminator::Authority)
    pub authority_type: u8,      // 1 (0=Ed25519, 1=Secp256r1)
    pub role: u8,                // 1 (0=Owner, 1=Admin, 2=Spender)
    pub bump: u8,                // 1
    pub wallet: Pubkey,          // 32 (Parent Wallet Link)
}
```

**2. Type-Specific Layouts**

*   **Type 0: Ed25519**
    *   **Layout**: `[ Header ] + [ Pubkey (32 bytes) ]`
    *   **Total Size**: 36 + 32 = **68 bytes**
    *   *Note*: Simple layout. No `credential_hash` or `odometer`.

*   **Type 1: Secp256r1 (Passkey)**
    *   **Layout**: `[ Header ] + [ Odometer (4 bytes) ] + [ CredentialHash (32 bytes) ] + [ Pubkey (Variable, ~33 bytes) ]`
    *   **Total Size**: 36 + 4 + 32 + 33 = **~105 bytes**
    *   *Additional Data*:
        *   `SignatureOdometer` (u32): Replay protection counter.
        *   `CredentialHash` (32): The constant ID used for PDA derivation (SHA256 of credential ID).
    *   *Identity*:
        *   `Pubkey`: The actual public key (variable length).

### C. Session Account (Ephemeral)
Temporary sub-key for automated agents/spenders.
*   **Seeds**: `[b"session", wallet_pubkey, session_key]`
*   **Space**: ~56 bytes.
*   **Data Structure**:
    ```rust
    #[repr(C, align(8))]
    #[derive(NoPadding)]
    pub struct SessionAccount {
        pub discriminator: u8,      // 1 (AccountDiscriminator::Session)
        pub bump: u8,               // 1
        pub _padding: [u8; 6],      // 6
        pub wallet: Pubkey,         // 32
        pub session_key: Pubkey,    // 32
        pub expires_at: i64,        // 8
    }
    ```

### D. Vault (Asset Holder)
The central SOL/Token container.
*   **Seeds**: `[b"vault", wallet_pubkey]`
*   **Data**: None.
*   **Owner**: System Program.
*   **Signing**: Signed via `invoke_signed` using seeds.

## 5. RBAC (Roles & Permissions)

| Role | ID | Permissions |
| :--- | :--- | :--- |
| **Owner** | 0 | **Superuser**. Can `AddAuthority` (Owners/Admins/Spenders), `RemoveAuthority`, `TransferOwnership`. |
| **Admin** | 1 | **Manager**. Can `Execute`, `CreateSession`. Can `AddAuthority`/`RemoveAuthority` **only for Spenders**. |
| **Spender** | 2 | **User**. Can `Execute` only. |

## 6. Detailed Instruction Logic

### 1. `CreateWallet`
*   **Accounts**:
    1.  `[Signer, Write] Payer`
    2.  `[Write] WalletPDA` (New)
    3.  `[Write] VaultPDA` (New)
    4.  `[Write] AuthorityPDA` (New - The Creator)
    5.  `[RO] SystemProgram`
*   **Arguments**:
    *   `user_seed: Vec<u8>`
    *   `auth_type: u8`
    *   **Variable Data**:
        *   **Ed25519**: `[Pubkey (32)]`.
        *   **Secp256r1**: `[CredentialHash (32)] + [Pubkey (33)]`.
*   **Logic**:
    1.  Parse args and variable data.
    2.  Derive seeds for Wallet `[b"wallet", user_seed]`. Create Account.
    2.  Derive seeds for Vault `[b"vault", wallet_key]`. Assign to System.
    3.  Init `WalletAccount` header.
    4.  Derive `AuthorityPDA`.
        *   If Ed25519: Seed = `auth_pubkey[0..32]`.
        *   If Secp256r1: Seed = `credential_hash`.
    5.  Create `AuthorityPDA`.
    6.  Init `AuthorityAccount`: `Role = Owner`, `Odometer = 0`, `Wallet = WalletPDA`.

### 2. `Execute`
*   **Accounts**:
    1.  `[Signer] Payer` (Gas Payer / Relayer)
    2.  `[RO] Wallet`
    3.  `[RO] Authority` (The Authenticated User PDA)
    4.  `[RO] Vault` (Signer for CPI)
    5.  `[RO] SysvarInstructions` (If Secp256r1)
    6.  `... [RO/Write] InnerAccounts` (Target programs/accounts)
*   **Arguments**:
    *   `instructions: CompactInstructions` (Compressed payload)
*   **Authentication Flow**:
    1.  **Check 0**: `Authority.wallet == Wallet`.
    2.  **Check 1 (Ed25519)**:
        *   If `Authority.type == 0`: Verify `Authority.pubkey` is a `Signer` on the transaction.
    3.  **Check 1 (Secp256r1)**:
        *   If `Authority.type == 1`:
        *   Load `SysvarInstructions`.
        *   Verify `Secp256r1` precompile instruction exists at specified index.
        *   Verify Precompile `pubkey` matches `Authority.pubkey`.
        *   Verify Precompile `message` matches expected hash of `instructions`.
        *   **Replay Check**: Verify `Precompile.message.counter > Authority.odometer`.
        *   **Update**: Set `Authority.odometer = Precompile.message.counter` (Authority must be Writable).
*   **Execution Flow**:
    1.  **Decompress**: Expand `CompactInstructions` into standard `Instruction` structs using `InnerAccounts`.
    2.  **Loop**: For each instruction:
        *   Call `invoke_signed(&instruction, accounts, &[vault_seeds])`.

### 3. `AddAuthority`
*   **Accounts**: `[Signer] Payer`, `[RO] Wallet`, `[RO] AdminAuth`, `[Write] NewAuthPDA`.
*   **Args**: `new_type`, `new_pubkey`, `new_hash`, `new_role`.
*   **Logic**:
    1.  **Auth**: Authenticate `AdminAuth` (Signer Check).
    2.  **Permission**:
        *   `AdminAuth.role == Owner` -> Allow Any.
        *   `AdminAuth.role == Admin` AND `new_role == Spender` -> Allow.
        *   Else -> Error `Unauthorized`.
    3.  **Create**: System Create `NewAuthPDA` with seeds `[b"authority", wallet, hash]`.
    4.  **Init**: Write data.

### 4. `RemoveAuthority`
*   **Accounts**: `[Signer] Payer`, `[RO] Wallet`, `[RO] AdminAuth`, `[Write] TargetAuth`, `[Write] RefundDest`.
*   **Logic**:
    1.  **Auth**: Authenticate `AdminAuth`.
    2.  **Permission**:
        *   `AdminAuth.role == Owner` -> Allow Any.
        *   `AdminAuth.role == Admin` AND `TargetAuth.role == Spender` -> Allow.
        *   Else -> Error `Unauthorized`.
    3.  **Close**: Zero data, transfer lamports to `RefundDest`.

### 5. `TransferOwnership` (Atomic Handover)
*   **Accounts**: `[Signer] Payer`, `[RO] Wallet`, `[Write] CurrentOwnerAuth`, `[Write] NewOwnerAuth`.
*   **Logic**:
    1.  **Auth**: Authenticate `CurrentOwnerAuth`. Must be `Role=Owner`.
    2.  **Create**: Init `NewOwnerAuth` with `Role=Owner`.
    3.  **Close**: Close `CurrentOwnerAuth`.
    *Result*: Ownership key rotated safely in one transaction.
