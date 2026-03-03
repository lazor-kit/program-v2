# ⚡ LazorKit Smart Wallet (V2)

**LazorKit** is a high-performance, security-focused Smart Wallet contract on Solana. It enables advanced account abstraction features like multi-signature support, session keys, and role-based access control (RBAC) with minimal on-chain overhead.

---

## 🌟 Key Features

### 🔐 Multi-Protocol Authentication
- **Ed25519**: Native Solana key support for standard wallets.
- **Secp256r1 (P-256)**: Native support for **Passkeys (WebAuthn)** and **Apple Secure Enclave**, enabling biometric signing directly on-chain.

### 🛡️ Role-Based Access Control (RBAC)
Granular permission management for every key with strictly separated PDAs:
- **Owner (Role 0)**: Full control. Can add/remove authorities and transfer ownership.
- **Admin (Role 1)**: Can create Sessions and add Spenders. Cannot remove Owners.
- **Spender (Role 2)**: Limited to executing transactions. Ideal for hot wallets or automated bots.

### ⏱️ Ephemeral Session Keys
- Create temporary, time-bound keys with specific expiry (`expires_at` defined by absolute slot height).
- Great for dApps (games, social) to offer "Log in once, act multiple times" UX without exposing the main key.

### 🚀 High Performance & Replay Protection
- **Zero-Copy Serialization**: Built on `pinocchio` casting raw bytes to Rust structs for maximum CU efficiency.
- **No-Padding Layout**: Optimized data structures (`NoPadding`) to reduce rent costs and ensure memory safety.
- **SlotHashes Nonce**: Secp256r1 replay protection uses the `SlotHashes` sysvar as a "Proof of Liveness" (valid within 150 slots) instead of expensive on-chain counters.
- **Transaction Compression**: Uses `CompactInstructions` to fit complex multi-call payloads into standard Solana transaction limits.

---

## 🏗️ Architecture

The contract uses a highly modular PDA (Program Derived Address) architecture for separated storage and deterministic validation:

| Account Type | Description |
| :--- | :--- |
| **Wallet PDA** | The main identity anchor. Derived from `["wallet", user_seed]`. |
| **Vault PDA** | Holds assets (SOL/SPL Tokens). Only the Wallet PDA can sign for it. |
| **Authority PDA** | Separate PDA for each authorized key (unlimited distinct authorities). Stores role. Derived from `["authority", wallet_pda, id_hash]`. |
| **Session PDA** | Temporary authority (sub-key) with absolute slot-based expiry. Derived from `["session", wallet_pda, session_key]`. |

*See [`docs/Architecture.md`](docs/Architecture.md) for deeper technical details.*

---

## 📂 Project Structure

- `program/src/`: Main contract source code.
  - `processor/`: Instruction handlers (`create_wallet`, `execute`, `manage_authority`, etc.).
  - `auth/`: Authentication logic for Ed25519 and Secp256r1 (with `slothashes` nonce).
  - `state/`: Account data structures (`Wallet`, `Authority`, `Session`).
- `tests-e2e/`: Comprehensive End-to-End Test Suite.
  - `scenarios/`: Test scenarios covering Happy Path, Failures, and Audit Retro.
  - `scenarios/audit/`: Dedicated regression tests for security vulnerabilities.

---

## 🛠️ Usage

### Build
```bash
# Build SBF program
cargo build-sbf
```

### Test
Run the comprehensive E2E test suite (LiteSVM-based):
```bash
cd tests-e2e
cargo run --bin lazorkit-tests-e2e
```

---

## 🔒 Security & Audit

LazorKit V2 has undergone a rigorous internal audit and security review. 

**Status**: ✅ **17/17 Security Issues Resolved**

We have fixed and verified vulnerabilities including:
- **Critical**: Cross-Wallet Authority Deletion.
- **High**: Signature Replay, DoS prevention, OOB Reads.
- **Medium**: Rent Theft protections and Signature Binding.
- **CPI Protection**: Explicit `stack_height` checks prevent authentication instructions from being called maliciously via CPI.

### Security Features
- **Discriminator Checks**: All PDAs are strictly validated by type constant.
- **Signature Binding**: Payloads are strictly bound to target accounts and instructions to prevent replay/swapping attacks.
- **Reentrancy Guards**: Initialized to prevent CPI reentrancy.

---

## 📜 License
MIT
