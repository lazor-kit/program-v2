# ⚡ LazorKit Smart Wallet (V2)

**LazorKit** is a high-performance, security-focused Smart Wallet contract on Solana. It enables advanced account abstraction features like multi-signature support, session keys, and role-based access control (RBAC) with minimal on-chain overhead.

---

## 🌟 Key Features

### 🔐 Multi-Protocol Authentication
- **Ed25519**: Native Solana key support for standard wallets.
- **Secp256r1 (P-256)**: Native support for **Passkeys (WebAuthn)** and **Apple Secure Enclave**, enabling biometric signing directly on-chain.

### 🛡️ Role-Based Access Control (RBAC)
Granular permission management for every key:
- **Owner (Role 0)**: Full control. Can add/remove authorities and transfer ownership.
- **Admin (Role 1)**: Can create Sessions and add Spenders. Cannot remove Owners.
- **Spender (Role 2)**: Limited to executing transactions. Ideal for hot wallets or automated bots.

### ⏱️ Ephemeral Session Keys
- Create temporary, time-bound keys with specific expiry (Slot Height).
- Great for dApps (games, social) to offer "Log in once, act multiple times" UX without exposing the main key.

### 🚀 High Performance
- **Zero-Copy Serialization**: Built on `pinocchio` for maximum CU efficiency.
- **No-Padding Layout**: Optimized data structures (`NoPadding`) to reduce rent costs.
- **Replay Protection**: Built-in counter system for Secp256r1 signatures to prevent double-spending attacks.

---

## 🏗️ Architecture

The contract uses a PDA (Program Derived Address) architecture to manage state:

| Account Type | Description |
| :--- | :--- |
| **Wallet PDA** | The main identity anchor. Derived from `["wallet", user_seed]`. |
| **Vault PDA** | Holds assets (SOL/SPL Tokens). Only the Wallet PDA can sign for it. |
| **Authority PDA** | Separate PDA for each authorized key (Device/User). Stores role & counter. Derived from `["authority", wallet_pda, key_or_hash]`. |
| **Session PDA** | Temporary authority derived from a session key and wallet. |

---

## 📂 Project Structure

- `program/src/`: Main contract source code.
  - `processor/`: Instruction handlers (`create_wallet`, `execute`, `manage_authority`, etc.).
  - `auth/`: Authentication logic for Ed25519 and Secp256r1.
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
- **Critical**: Cross-Wallet Authority Deletion (Issue #3).
- **High**: Signature Replay (Issues #16, #13, #11), DoS prevention (Issue #4), OOB Reads (Issue #17).
- **Medium**: Rent Theft protections (Issue #14) and Signature Binding (Issues #8, #9).

👉 **[View Full Audit Report](Report.md)**

### Security Features
- **Discriminator Checks**: All PDAs are strictly validated by type constant.
- **Signature Binding**: Payloads are strictly bound to target accounts and instructions to prevent replay/swapping attacks.
- **Reentrancy Guards**: Initialized to prevent CPI reentrancy.

---

## 📜 License
MIT
