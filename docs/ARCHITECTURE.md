# LazorKit Wallet Contract - Architecture v3.0.0

**Version**: 3.0.0  
**Last Updated**: 2026-01-20  
**Status**: Simplified Architecture - Production Ready

---

## Overview

LazorKit is a simplified smart contract wallet on Solana featuring **Role-Based Access Control (RBAC)** and **Session Keys**. Version 3.0.0 focuses on a clean, minimal implementation supporting only essential authority types and a straightforward permission hierarchy.

### Key Features

- ✅ **Role-Based Access Control**: Owner, Admin, and Spender roles
- ✅ **Session Keys**: Temporary keys with expiration for Ed25519 and Secp256r1
- ✅ **Multi-Authority**: Multiple signers per wallet
- ✅ **Zero-Copy Design**: Efficient state management without serialization overhead
- ✅ **CPI Support**: Execute transactions on behalf of the wallet

---

## 1. Authority Types

LazorKit v3.0.0 supports **4 authority types** based on 2 cryptographic standards:

| Type | Code | Size | Description |
|------|------|------|-------------|
| **Ed25519** | 1 | 32 bytes | Standard Solana keypair |
| **Ed25519Session** | 2 | 80 bytes | Ed25519 with session key support |
| **Secp256r1** | 5 | 40 bytes | Passkey/WebAuthn compatible (compressed) |
| **Secp256r1Session** | 6 | 88 bytes | Secp256r1 with session key support |

### 1.1 Ed25519 Authority

**Size**: 32 bytes  
**Layout**:
```
[0..32]  public_key: [u8; 32]
```

**Authentication**: Standard Ed25519 signature verification using Solana's native `ed25519_program`.

---

### 1.2 Ed25519Session Authority

**Size**: 80 bytes  
**Layout**:
```
[0..32]   master_key: [u8; 32]
[32..64]  session_key: [u8; 32]
[64..72]  max_session_length: u64 (slots)
[72..80]  current_session_expiration: u64 (slot number)
```

**Authentication Flow**:
1. Check if current slot < `current_session_expiration`
2. If session active: verify signature with `session_key`
3. If session expired: verify signature with `master_key`

**Creating Session**:
- Requires master key signature
- Sets `current_session_expiration = current_slot + duration`
- Duration must be ≤ `max_session_length`

---

### 1.3 Secp256r1 Authority  

**Size**: 40 bytes (compressed public key + metadata)  
**Layout**:
```
[0..33]   compressed_pubkey: [u8; 33]
[33..40]  last_signature_slot: u64
```

**Authentication**: WebAuthn-style signature verification
- Prevents signature replay via `last_signature_slot` tracking
- Signature must be more recent than last known slot
- Max signature age: 150 slots (~1 minute)

---

### 1.4 Secp256r1Session Authority

**Size**: 88 bytes  
**Layout**:
```
[0..33]   master_compressed_pubkey: [u8; 33]
[33..65]  session_key: [u8; 32]
[65..73]  max_session_length: u64
[73..81]  current_session_expiration: u64
[81..88]  last_signature_slot: u64
```

**Authentication**: Same as Secp256r1, with session key support

---

## 2. Role Storage Structure

Roles are stored in a **dynamic buffer** following the wallet header. Each role consists of:

```
┌─────────────────┬──────────────────┐
│ Position Header │ Authority Data   │
│    (16 bytes)   │  (variable size) │
└─────────────────┴──────────────────┘
```

### 2.1 Position Header

**Size**: 16 bytes (8-byte aligned)  
**Layout**:
```rust
pub struct Position {
    authority_type: u16,     // Type code (1, 2, 5, or 6)
    authority_length: u16,   // Authority data size in bytes  
    _padding: u32,           // For alignment
    id: u32,                 // Role ID (0=Owner, 1=Admin, 2+=Spender)
    boundary: u32,           // Absolute offset to next role
}
```

### 2.2 Complete Account Layout

```
┌──────────────────────┬────────────────────┬─────────────┬─────────────┐
│ LazorKitWallet (48)  │ Role 0 (Owner)     │ Role 1 (...)│  Role N     │
├──────────────────────┼────────────────────┼─────────────┼─────────────┤
│ - discriminator (1)  │ Position (16)      │ ...         │ ...         │
│ - role_count (4)     │ Authority (varies) │             │             │
│ - role_counter (4)   │                    │             │             │
│ - vault_bump (1)     │                    │             │             │
│ - padding (38)       │                    │             │             │
└──────────────────────┴────────────────────┴─────────────┴─────────────┘
```

---

## 3. RBAC (Role-Based Access Control)

LazorKit uses a **3-tier permission hierarchy** based on role IDs:

| Role ID | Name | Permissions |
|---------|------|-------------|
| **0** | Owner | Full control + Transfer ownership |
| **1** | Admin | Add/remove authorities, create sessions |
| **2+** | Spender | Execute transactions, create sessions |

### 3.1 Permission Matrix

| Operation | Owner (0) | Admin (1) | Spender (2+) |
|-----------|-----------|-----------|--------------|
| Execute transactions | ✅ | ✅ | ✅ |
| Create session | ✅ | ✅ | ✅ |
| Add authority | ✅ | ✅ | ❌ |
| Remove authority | ✅ | ✅ | ❌ |
| Update authority | ✅ | ✅ | ❌ |
| Transfer ownership | ✅ | ❌ | ❌ |

### 3.2 Anti-Lockout Protection

**Cannot remove last admin**: If removing a role with `id == 1`, the contract checks that at least one other admin (`id == 1`) exists.

---

## 4. Instructions

LazorKit v3.0.0 implements **7 instructions**:

| Discriminator | Instruction | Description |
|---------------|-------------|-------------|
| 0 | CreateWallet | Initialize new wallet with Owner |
| 1 | AddAuthority | Add new role (Owner/Admin only) |
| 2 | RemoveAuthority | Remove existing role (Owner/Admin only) |
| 3 | UpdateAuthority | Update authority data for existing role |
| 4 | CreateSession | Create temporary session key |
| 5 | Execute | Execute CPI on behalf of wallet |
| 6 | TransferOwnership | Transfer Owner role to new authority |

### 4.1 CreateWallet

**Discriminator**: 0  
**Accounts**:
- `[writable, signer]` Config - PDA to initialize
- `[signer]` Payer - Fee payer
- `[]` System Program

**Args**:
```rust
{
    authority_type: u16,
    authority_data: Vec<u8>,
    id: Vec<u8>, // Unique identifier for wallet PDA
}
```

**Description**: Creates a new wallet with the first authority as Owner (role ID 0).

---

### 4.2 AddAuthority

**Discriminator**: 1  
**Accounts**:
- `[writable, signer]` Config
- `[signer]` Payer
- `[]` System Program

**Args**:
```rust
{
    acting_role_id: u32,
    new_authority_type: u16,
    new_authority_data: Vec<u8>,
    authorization_data: Vec<u8>,
}
```

**Permission**: Owner (0) or Admin (1) only

---

### 4.3 RemoveAuthority

**Discriminator**: 2  
**Accounts**: Same as AddAuthority

**Args**:
```rust
{
    acting_role_id: u32,
    target_role_id: u32,
    authorization_data: Vec<u8>,
}
```

**Permission**: Owner (0) or Admin (1) only  
**Restrictions**:
- Cannot remove Owner (role 0)
- Cannot remove last Admin (if target has id==1)

---

### 4.4 UpdateAuthority

**Discriminator**: 3  
**Accounts**: Same as AddAuthority

**Args**:
```rust
{
    acting_role_id: u32,
    target_role_id: u32,
    new_authority_data: Vec<u8>,
    authorization_data: Vec<u8>,
}
```

**Permission**: Owner (0) or Admin (1) only  
**Use Case**: Rotate keys, update session limits without removing/re-adding role

---

### 4.5 CreateSession

**Discriminator**: 4  
**Accounts**:
- `[writable, signer]` Config
- `[signer]` Payer
- `[]` System Program

**Args**:
```rust
{
    role_id: u32,
    session_key: [u8; 32],
    duration: u64, // in slots
    authorization_data: Vec<u8>,
}
```

**Permission**: Any role (must authenticate with master key)  
**Requirement**: Authority type must support sessions (Ed25519Session or Secp256r1Session)

---

### 4.6 Execute

**Discriminator**: 5  
**Accounts**:
- `[writable, signer]` Config
- `[]` Vault (wallet PDA)
- `[...]` Remaining accounts passed to target program

**Args**:
```rust
{
    role_id: u32,
    target_program: Pubkey,
    data: Vec<u8>,          // Instruction data for target
    account_metas: Vec<u8>, // Serialized AccountMeta list
    authorization_data: Vec<u8>,
}
```

**Permission**: All roles  
**Description**: Executes Cross-Program Invocation (CPI) with Vault as signer

---

### 4.7 TransferOwnership

**Discriminator**: 6  
**Accounts**:
- `[writable, signer]` Config
- `[signer]` Owner (current role 0)

**Args**:
```rust
{
    new_owner_authority_type: u16,
    new_owner_authority_data: Vec<u8>,
    auth_payload: Vec<u8>,
}
```

**Permission**: Owner (0) only  
**Restriction**: New authority size must match current Owner size (no data migration)

---

## 5. PDA Derivation

### 5.1 Config Account

**Seeds**: `["lazorkit", id]`

```rust
let (config_pda, bump) = Pubkey::find_program_address(
    &[b"lazorkit", id.as_bytes()],
    &program_id
);
```

### 5.2 Vault Account (Wallet Address)

**Seeds**: `["lazorkit-wallet-address", config_pubkey]`

```rust
let (vault_pda, vault_bump) = Pubkey::find_program_address(
    &[b"lazorkit-wallet-address", config_pda.as_ref()],
    &program_id
);
```

The `vault_bump` is stored in `LazorKitWallet.vault_bump` for efficient re-derivation.

---

## 6. Error Codes

### Authentication Errors (3000+)

| Code | Error | Description |
|------|-------|-------------|
| 3000 | InvalidAuthority | Invalid or missing authority |
| 3001 | InvalidAuthorityPayload | Malformed signature data |
| 3014 | PermissionDeniedSessionExpired | Session key expired |
| 3034 | InvalidSessionDuration | Duration exceeds max_session_length |

### State Errors (1000+)

| Code | Error | Description |
|------|-------|-------------|
| 1000 | InvalidAccountData | Corrupted account data |
| 1002 | InvalidAuthorityData | Malformed authority structure |
| 1004 | RoleNotFound | Role ID does not exist |

---

## 7. Zero-Copy Design

LazorKit uses **zero-copy** techniques for performance:

- No Borsh serialization/deserialization after initialization
- Direct byte manipulation via `load_unchecked()` and `load_mut_unchecked()`
- In-place updates to account data
- Fixed-size headers with variable-length authority sections

**Example**:
```rust
// Load authority data directly from account slice
let auth = unsafe { 
    Ed25519Authority::load_mut_unchecked(&mut data[offset..offset+32])? 
};
auth.authenticate(accounts, signature, payload, slot)?;
```

---

## 8. Security Considerations

### 8.1 Signature Replay Protection

**Secp256r1/Secp256r1Session**:
- Tracks `last_signature_slot` to prevent reuse
- Enforces max signature age (150 slots)

**Ed25519/Ed25519Session**:
- Uses Solana's native Ed25519 program with SigVerify checks
- Implicit replay protection via instruction sysvar

### 8.2 Session Security

- Sessions require master key signature to create
- Automatic expiration via slot number comparison
- Falls back to master key when session expires
- Cannot extend session without master key

### 8.3 Permission Isolation

- Role-based checks in every operation
- Cannot escalate privileges (Spender → Admin)
- Owner transfer requires explicit signature
- Anti-lockout via last-admin protection

---

## 9. Upgrade from v2.x

Version 3.0.0 is a **breaking change** from v2.x. Key differences:

| Feature | v2.x | v3.0.0 |
|---------|------|--------|
| Authority types | 8 types | 4 types (Ed25519, Secp256r1 + sessions) |
| RBAC | Plugin/policy-based | Simple role ID hierarchy |
| Permissions | Action-based | Role-based |
| Plugin system | ✅ | ❌ (Removed) |
| Multisig | ✅ | ❌ (Use multiple admins instead) |

**Migration**: Not supported. Deploy new wallet and transfer assets.

---

## 10. Examples

### Creating Wallet with Ed25519

```rust
let authority_data = owner_keypair.pubkey().to_bytes();
let id = b"my-wallet".to_vec();

let ix = LazorKitInstruction::CreateWallet {
    authority_type: 1, // Ed25519
    authority_data: authority_data.to_vec(),
    id,
};
```

### Adding Admin Role

```rust
let ix = LazorKitInstruction::AddAuthority {
    acting_role_id: 0, // Owner
    new_authority_type: 2, // Ed25519Session
    new_authority_data: admin_data,
    authorization_data: owner_signature,
};
```

### Executing Transaction

```rust
let ix = LazorKitInstruction::Execute {
    role_id: 2, // Spender
    target_program: spl_token::id(),
    data: transfer_instruction_data,
    account_metas: serialized_accounts,
    authorization_data: spender_signature,
};
```

---

## Appendix A: Authority Data Formats

### Ed25519 Creation Data
```
[32 bytes] public_key
```

### Ed25519Session Creation Data
```
[32 bytes] master_public_key
[32 bytes] initial_session_key
[8 bytes]  max_session_length (u64)
```

### Secp256r1 Creation Data
```
[33 bytes] compressed_public_key
```

### Secp256r1Session Creation Data
```
[33 bytes] master_compressed_public_key
[32 bytes] initial_session_key
[8 bytes]  max_session_length (u64)
```

---

**End of Architecture Document**
