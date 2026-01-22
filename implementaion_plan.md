# LazorKit Detailed Implementation Master Plan

## Project Goal
Build a high-performance, secure smart wallet on Solana using `pinocchio` (zero-copy), supporting Passkeys (Secp256r1), separated PDA storage, and transaction compression.

---

## Phase 1: Foundation & State Infrastructure

**Goal**: Setup the workspace and implement the core zero-copy safety tools and PDA state definitions.

### 1.1 Workspace Setup [DONE]
*   **Action**: Create `program/` directory with standard Solana layout.
*   **File**: `program/Cargo.toml`
    *   Add `pinocchio`, `pinocchio-pubkey`, `pinocchio-system`.
    *   Add `[lib] crate-type = ["cdylib", "lib"]`.
*   **File**: `Cargo.toml` (Root)
    *   Setup workspace members.

### 1.2 Zero-Copy Utilities (The "Swig" Port) [DONE]
*   **Component**: `NoPadding` Macro
    *   **Source to Learn**: `swig-wallet/no-padding/src/lib.rs`
    *   **Action**: Implement a proc-macro that inspects struct fields and generates a `const _: () = { assert!(size == sum_fields); }` block to ensure no compiler padding exists.
*   **Component**: `Assertions`
    *   **Source to Learn**: `swig-wallet/assertions/src/lib.rs`
    *   **Action**: Implement optimized wrappers for `sol_memcmp` syscalls to save Compute Units.

### 1.3 State Definitions (PDAs) [DONE]
*   **File**: `program/src/state/wallet.rs`
    *   **Struct**: `WalletAccount`
    *   **Fields**: `bump: u8`, `padding: [u8; 7]`.
    *   **Learn**: How Swig uses `#[repr(C, align(8))]` with `NoPadding`.
*   **File**: `program/src/state/authority.rs`
    *   **Struct**: `AuthorityAccount`
    *   **Fields**:
        *   `discriminator: u64`
        *   `wallet: Pubkey`
        *   `authority_type: u8`
        *   `role: u8`
        *   `bump: u8`
        *   `padding_1: [u8; 5]`
        *   `signature_odometer: u32`
        *   `padding_2: [u8; 4]`
        *   `pubkey: [u8; 33]`
        *   `padding_3: [u8; 7]`
        *   `credential_hash: [u8; 32]`
*   **File**: `program/src/state/session.rs`
*   **Source to Learn**: `swig-wallet/state/src/role.rs` (though we simplify the logic, the layout principles apply).

---

## Phase 2: Authentication Engine (The Core)

**Goal**: Implement the cryptographic verification logic.

### 2.1 Ed25519 Authentication Verification [DONE]
*   **File**: `program/src/auth/ed25519.rs`
*   **Source to Learn**: `swig-wallet/state/src/authority/ed25519.rs`
*   **Logic**:
    1.  Read `auth_pda.pubkey`.
    2.  Scan `accounts` (Signers) to find one matching this pubkey.
    3.  Assert `is_signer` is true.

### 2.2 Secp256r1 (Passkey) Authentication [DONE]
*   **File**: `program/src/auth/secp256r1/mod.rs`
*   **Source to Learn**: `swig-wallet/state/src/authority/secp256r1.rs` (The most critical file).
*   **Sub-component**: Odometer
    *   **Learn**: How Swig checks `counter > odometer` to prevent replay.
    *   **Logic**: `assert(payload.counter > auth_pda.odometer)`.
    *   **Logic**: `auth_pda.odometer = payload.counter`.
*   **Sub-component**: Precompile Introspection
    *   **File**: `program/src/auth/secp256r1/introspection.rs`
    *   **Learn**: How `swig-wallet` parses `Secp256r1SignatureOffsets` from `sysvar::instructions`.
    *   **Logic**: Use `sysvar::instructions` to find the `Secp256r1` instruction.
    *   **Verify**: `instruction.data` contains the expected `pubkey` and `message_hash`.
*   **Sub-component**: WebAuthn Parsing
    *   **File**: `program/src/auth/secp256r1/webauthn.rs`
    *   **Learn**: Deeply study `webauthn_message` and `decode_huffman_origin` in Swig.
    *   **Logic**: Implement Huffman decoding for `clientDataJSON`.
    *   **Logic**: Reconstruct `authenticatorData` to verify the hash.

---

## Phase 3: Instructions & Management

**Goal**: Implement the user-facing instructions.

### 3.1 `CreateWallet`
*   **File**: `program/src/processor/create_wallet.rs`
*   **Accounts**: Payer, Wallet, Vault, Authority, System.
*   **Steps**:
    1.  `invoke(system_instruction::create_account)` for Wallet.
    2.  Write Wallet discriminators.
    3.  `invoke(system_instruction::create_account)` for Authority.
    4.  Write Authority discriminators (Role=Owner).

### 3.2 `AddAuthority` & `RemoveAuthority`
*   **File**: `program/src/processor/manage_authority.rs`
*   **Logic**:
    *   Load `SignerAuthority`. Authenticate (Phase 2).
    *   Check RBAC: `if Signer.Role == Admin { assert(Target.Role == Spender) }`.
    *   `Add`: Create PDA. `Remove`: Close PDA/Transfer Lamports.

### 3.3 `TransferOwnership`
*   **File**: `program/src/processor/transfer_ownership.rs`
*   **Logic**: Atomic swap. Add new Owner auth, close current Owner auth.

---

## Phase 4: Execution Engine (Compressed)

**Goal**: Implement the transaction runner.

### 4.1 Compact Instructions SerDe
*   **File**: `program/src/instructions/compact.rs`
*   **Source to Learn**: `swig-wallet/instructions/src/compact_instructions.rs`
*   **Struct**: `CompactInstruction { program_id_index: u8, account_indexes: Vec<u8>, data: Vec<u8> }`
*   **Logic**:
    *   Input: `serialized_compact_bytes`.
    *   Decompress: Map `index` -> `account_info_key`.
    *   Output: `Instruction` struct ready for CPI.

### 4.2 `Execute` Instruction
*   **File**: `program/src/processor/execute.rs`
*   **Source to Learn**: `swig-wallet/program/src/actions/sign_v2.rs` (especially `InstructionIterator`).
*   **Steps**:
    1.  **Auth**: Call Phase 2 Auth logic.
    2.  **Role**: Assert `Authority.role` is valid.
    3.  **Decompress**: Parse args into Instructions.
    4.  **Loop**:
        *   `invoke_signed(instruction, accounts, &[vault_seeds])`.

---

## Phase 5: Verification & Testing

**Goal**: Ensure it works and is secure.

### 5.1 Unit Tests (Rust)
*   **Target**: `NoPadding` macro (ensure it fails on padded structs).
*   **Target**: `CompactInstructions` serialisation.
*   **Target**: WebAuthn parser (feed real Passkey outputs).

### 5.2 Integration Tests (`solana-program-test`)
*   **File**: `tests/integration_tests.rs`
*   **Scenario A: Lifecycle**: Create Wallet -> Add Admin -> Add Spender -> Spender Executes -> Owner Removes Admin.
*   **Scenario B: Passkey**: Mock a Secp256r1 precompile (or simulated signature ok) and verify `odometer` increments.
*   **Scenario C: Limits**: Verify transaction size limits with/without compression to prove benefit.
