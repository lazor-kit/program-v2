# LazorKit Wallet Contract

> **Smart Contract Wallet on Solana with Role-Based Access Control and Session Keys**

LazorKit is an optimized smart contract wallet for Solana, featuring role-based access control (RBAC) and session keys for enhanced security and user experience.

## ✨ Key Features

- 🔐 **Role-Based Access Control (RBAC)**: Clear permission hierarchy with 3 roles (Owner, Admin, Spender)
- 🔑 **Session Keys**: Create temporary keys with expiration, no master key needed for every transaction
- 🌐 **WebAuthn/Passkey Support**: Secp256r1 integration for passkey and biometric authentication
- ⚡ **Zero-Copy Design**: High performance with zero-copy design, no serialization overhead
- 🛡️ **Anti-Lockout Protection**: Cannot remove last admin, ensuring wallet always has management access
- 🔄 **CPI Support**: Execute transactions on behalf of wallet via Cross-Program Invocation

## 🏗️ Architecture

```
┌─────────────────────────────────────────────────┐
│           LazorKit Wallet Account               │
├─────────────────────────────────────────────────┤
│  Header (48 bytes)                              │
│  - discriminator, role_count, vault_bump        │
├─────────────────────────────────────────────────┤
│  Role 0: Owner (16 + authority data)            │
│  - Position header + Authority                  │
├─────────────────────────────────────────────────┤
│  Role 1: Admin (16 + authority data)            │
├─────────────────────────────────────────────────┤
│  Role 2+: Spender (16 + authority data)         │
└─────────────────────────────────────────────────┘
```

🔗 **Technical Details**: See [ARCHITECTURE.md](docs/ARCHITECTURE.md)

## 🚀 Quick Start

### Prerequisites

- Rust 1.75+
- Solana CLI 1.18+
- Anchor (optional)

### Build Contract

```bash
# Build for Solana BPF
cd contracts/program
cargo build-sbf

# Output: target/deploy/lazorkit_program.so
```

### Deploy

```bash
# Deploy to devnet
solana program deploy \
  target/deploy/lazorkit_program.so \
  --url devnet \
  --keypair ~/.config/solana/id.json

# Or use existing program ID
# LazorKit11111111111111111111111111111111111
```

## 📖 Usage

### 1. Create Wallet

```rust
use lazorkit::instruction::CreateWallet;

// Create wallet with Ed25519 owner
let ix = CreateWallet {
    authority_type: 1,  // Ed25519
    authority_data: owner_pubkey.to_bytes().to_vec(),
    id: b"my-wallet".to_vec(),
};
```

### 2. Add Admin

```rust
// Owner adds Admin role
let ix = AddAuthority {
    acting_role_id: 0,  // Owner
    new_authority_type: 2,  // Ed25519Session
    new_authority_data: admin_data,
    authorization_data: owner_signature,
};
```

### 3. Create Session Key

```rust
// Create session key for 1 hour (3600 slots)
let ix = CreateSession {
    role_id: 0,
    session_key: temp_keypair.pubkey().to_bytes(),
    duration: 3600,
    authorization_data: master_signature,
};
```

### 4. Execute Transaction

```rust
// Execute SOL transfer through wallet
let ix = Execute {
    role_id: 0,
    target_program: system_program::ID,
    data: transfer_instruction_data,
    account_metas: serialized_accounts,
    authorization_data: signature,
};
```

## 🔑 Authority Types

| Type | Code | Size | Description |
|------|------|------|-------------|
| **Ed25519** | 1 | 32 bytes | Standard Solana keypair |
| **Ed25519Session** | 2 | 80 bytes | Ed25519 + session key |
| **Secp256r1** | 5 | 40 bytes | WebAuthn/Passkey |
| **Secp256r1Session** | 6 | 88 bytes | Secp256r1 + session key |

## 👥 Permission Matrix

| Operation | Owner (0) | Admin (1) | Spender (2+) |
|-----------|-----------|-----------|--------------|
| Execute transactions | ✅ | ✅ | ✅ |
| Create session | ✅ | ✅ | ✅ |
| Add/Remove authority | ✅ | ✅ | ❌ |
| Update authority | ✅ | ✅ | ❌ |
| Transfer ownership | ✅ | ❌ | ❌ |

## 📂 Project Structure

```
wallet-management-contract/
├── contracts/
│   ├── program/       # Main contract logic
│   ├── state/         # State structs & authority types
│   ├── no-padding/    # Procedural macros
│   └── assertions/    # Helper assertions
├── docs/
│   └── ARCHITECTURE.md  # Technical specification
├── target/deploy/
│   └── lazorkit_program.so  # Compiled program
└── README.md          # This file
```

## 🔧 Development

### Test

```bash
# Unit tests
cargo test --workspace

# Integration tests (requires SDK)
# Note: SDK has been removed, build separately if needed
```

### Lint & Format

```bash
# Format code
cargo fmt

# Check lints
cargo clippy --all-targets
```

## 📝 Instructions

LazorKit supports 7 instructions:

| Discriminator | Instruction | Description |
|---------------|-------------|-------------|
| 0 | `CreateWallet` | Initialize new wallet |
| 1 | `AddAuthority` | Add new role |
| 2 | `RemoveAuthority` | Remove role |
| 3 | `UpdateAuthority` | Update authority data |
| 4 | `CreateSession` | Create temporary session key |
| 5 | `Execute` | Execute CPI |
| 6 | `TransferOwnership` | Transfer Owner to new authority |

## 🛡️ Security Features

### Replay Protection

- **Ed25519**: Native Solana signature verification
- **Secp256r1**: Counter-based + slot age validation (60 slots window)

### Session Security

- Requires master key signature to create session
- Automatic expiration based on slot number
- Cannot extend session without master key

### Permission Isolation

- Cannot self-escalate privileges (Spender → Admin)
- Owner transfer requires explicit signature
- Anti-lockout protection (cannot remove last admin)

## 📜 License

AGPL-3.0 - See [LICENSE](LICENSE)

## 🤝 Contributing

Contributions are welcome! Please:
1. Fork repo
2. Create feature branch
3. Commit changes
4. Open pull request

## 📞 Support

- **Documentation**: [docs/ARCHITECTURE.md](docs/ARCHITECTURE.md)
- **Issues**: GitHub Issues
- **Program ID**: `LazorKit11111111111111111111111111111111111`

---

**Built with ❤️ for Solana ecosystem**
