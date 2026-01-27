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
- **Spender (Role 2)**: Limited to executing transactions. ideal for hot wallets or automated bots.

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
| **Wallet PDA** | The main identity anchor. |
| **Vault PDA** | Holds assets (SOL/SPL Tokens). Only the Wallet PDA can sign for it. |
| **Authority PDA** | Separate PDA for each authorized key (Device/User). Stores role & counter. |
| **Session PDA** | Temporary authority derived from a session key and wallet. |

---

## 🛠️ Usage

### Build & Test
```bash
# Build SBF program
cargo build-sbf

# Run E2E Test Suite (Devnet)
cd tests-e2e
cargo run --bin lazorkit-tests-e2e
```

### Deployment (Devnet)
Currently deployed at:
> **Program ID**: `2r5xXopRxWYcKHVrrzGrwfRJb3N2DSBkMgG93k6Z8ZFC`

---

## 🔒 Security

- **Audited Logic**: Comprehensive checks for Replay Attacks, Privilege Escalation, and Memory Alignment.
- **Version Control**: Built-in Schema Versioning (V1) for future-proof upgrades.
- **Safe Math**: Strict arithmetic checks for all balance operations.

---

## 📜 License
MIT
