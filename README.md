# LazorKit - Open-source Smart Wallet Program on Solana

A comprehensive Solana-based smart wallet management system that provides secure passkey authentication, customizable policy engines, and flexible transaction execution capabilities.

## Overview

LazorKit is a sophisticated smart wallet system built on Solana that enables users to create and manage smart wallets with advanced security features:

- **Passkey Authentication**: Secure authentication using secp256r1 WebAuthn credentials
- **Policy Engine System**: Customizable transaction policies with a default policy implementation
- **Smart Wallet Management**: Create and manage smart wallets with passkey authentication
- **Chunk-Based Execution**: Execute complex transactions using deferred execution chunks
- **Multi-Device Support**: Support for multiple passkey devices per wallet

## Architecture

### Programs

The system consists of two main Solana programs:

#### 1. LazorKit Program (`Gsuz7YcA5sbMGVRXT3xSYhJBessW4xFC4xYsihNCqMFh`)

The core smart wallet program that handles:

- Smart wallet creation and initialization
- Passkey authentication management
- Transaction execution with authentication
- Chunk-based deferred execution for complex transactions

**Key Instructions:**

- `create_smart_wallet` - Create a new smart wallet with passkey
- `execute` - Execute transactions with passkey authentication
- `create_chunk` - Create a deferred execution chunk for complex transactions
- `execute_chunk` - Execute a previously created chunk (no authentication needed)
- `close_chunk` - Close a chunk and refund rent (no authentication needed)
- `delete_smart_wallet` - Delete a smart wallet and reclaim rent (program-level instruction, not exposed in SDK)

#### 2. Default Policy Program (`BiE9vSdz9MidUiyjVYsu3PG4C1fbPZ8CVPADA9jRfXw7`)

A reference implementation of transaction policies that provides:

- Policy initialization and validation
- Transaction checking and approval logic

**Key Instructions:**

- `init_policy` - Initialize policy for a smart wallet
- `check_policy` - Validate transaction against policies

### Contract Integration SDK

The `sdk` folder provides a comprehensive TypeScript SDK for interacting with the LazorKit system:

```
sdk/
├── anchor/           # Generated Anchor types and IDL
│   ├── idl/         # JSON IDL files
│   └── types/       # TypeScript type definitions
├── client/           # Main client classes
│   ├── lazorkit.ts  # Main LazorkitClient
│   ├── defaultPolicy.ts # DefaultPolicyClient
│   └── internal/    # Shared helpers
│       ├── walletPdas.ts # Centralized PDA derivation
│       ├── policyResolver.ts # Policy instruction resolver
│       └── cpi.ts    # CPI utilities
├── pda/             # PDA derivation functions
│   ├── lazorkit.ts  # Lazorkit PDA functions
│   └── defaultPolicy.ts # Default policy PDA functions
├── webauthn/        # WebAuthn/Passkey utilities
│   └── secp256r1.ts # Secp256r1 signature verification
├── auth.ts          # Authentication utilities
├── transaction.ts   # Transaction building utilities
├── utils.ts         # General utilities
├── validation.ts    # Validation helpers
├── messages.ts      # Message building utilities
├── constants.ts     # Program constants
├── types.ts         # TypeScript type definitions
├── index.ts         # Main exports
└── README.md        # SDK documentation
```

## Installation

### Prerequisites

- Node.js 
- Solana CLI
- Anchor Framework (v0.31.0)
- Rust (for program development)

### Setup

1. Clone the repository:

```bash
git clone https://github.com/lazor-kit/program-v2.git
cd program-v2
```

2. Install dependencies:

```bash
yarn install
```

3. Build the programs:

```bash
anchor build
```

## Program Deployment

### Deploy to Devnet

```bash
# Deploy LazorKit program
anchor deploy --provider.cluster devnet

# Deploy Default Policy program
anchor deploy --provider.cluster devnet --program-name default_policy
```

### Initialize IDL

```bash
# Initialize IDL for LazorKit
anchor idl init -f ./target/idl/lazorkit.json Gsuz7YcA5sbMGVRXT3xSYhJBessW4xFC4xYsihNCqMFh

# Initialize IDL for Default Policy
anchor idl init -f ./target/idl/default_policy.json BiE9vSdz9MidUiyjVYsu3PG4C1fbPZ8CVPADA9jRfXw7
```

### Upgrade IDL

```bash
# Initialize IDL for LazorKit
anchor idl upgrade Gsuz7YcA5sbMGVRXT3xSYhJBessW4xFC4xYsihNCqMFh -f ./target/idl/lazorkit.json

# Initialize IDL for Default Policy
anchor idl upgrade BiE9vSdz9MidUiyjVYsu3PG4C1fbPZ8CVPADA9jRfXw7 -f ./target/idl/default_policy.json
```

## SDK Usage

### Basic Setup

```typescript
import { LazorkitClient, DefaultPolicyClient } from './sdk';
import { Connection } from '@solana/web3.js';

// Initialize connection
const connection = new Connection('YOUR_RPC_URL');

// Create clients
const lazorkitClient = new LazorkitClient(connection);
const defaultPolicyClient = new DefaultPolicyClient(connection);
```

### Creating a Smart Wallet

```typescript
import { BN } from '@coral-xyz/anchor';

// Create smart wallet with passkey
const { transaction, smartWalletId, smartWallet } =
  await lazorkitClient.createSmartWalletTxn({
    payer: payer.publicKey,
    passkeyPublicKey: [
      /* 33 bytes */
    ],
    credentialIdBase64: 'base64-credential',
    amount: new BN(0.01 * 1e9), // Optional: initial funding in lamports
    policyInstruction: null, // Optional: policy initialization instruction
  });
```

### Executing Transactions

```typescript
// Execute transaction with authentication
const transaction = await lazorkitClient.executeTxn({
  payer: payer.publicKey,
  smartWallet: smartWallet.publicKey,
  passkeySignature: {
    passkeyPublicKey: [
      /* 33 bytes */
    ],
    signature64: 'base64-signature',
    clientDataJsonRaw64: 'base64-client-data',
    authenticatorDataRaw64: 'base64-auth-data',
  },
  credentialHash: [/* 32 bytes */],
  policyInstruction: null, // Optional: use default policy check if null
  cpiInstruction: transferInstruction,
  timestamp: new BN(Math.floor(Date.now() / 1000)),
  smartWalletId: walletStateData.walletId,
}, {
  computeUnitLimit: 200000, // Optional: set compute unit limit
  useVersionedTransaction: true, // Optional: use versioned transactions
});
```

### Managing Policies

Policy management is done through the policy program directly. The default policy handles device management and transaction validation:

```typescript
// Get required PDAs for policy initialization
const walletStateData = await lazorkitClient.getWalletStateData(smartWallet);
const policySigner = lazorkitClient.getWalletDevicePubkey(smartWallet, credentialHash);
const walletState = lazorkitClient.getWalletStatePubkey(smartWallet);

// Initialize policy during wallet creation
const initPolicyIx = await defaultPolicyClient.buildInitPolicyIx({
  walletId: walletStateData.walletId,
  passkeyPublicKey: passkeyPublicKey,
  credentialHash: credentialHash,
  policySigner: policySigner,
  smartWallet: smartWallet,
  walletState: walletState,
});

// Include policy initialization when creating wallet
const { transaction } = await lazorkitClient.createSmartWalletTxn({
  payer: payer.publicKey,
  passkeyPublicKey: [/* 33 bytes */],
  credentialIdBase64: 'base64-credential',
  policyInstruction: initPolicyIx,
});

// Check policy before executing transactions
const checkPolicyIx = await defaultPolicyClient.buildCheckPolicyIx({
  walletId: walletStateData.walletId,
  passkeyPublicKey: passkeyPublicKey,
  policySigner: policySigner,
  smartWallet: smartWallet,
  credentialHash: credentialHash,
  policyData: walletStateData.policyData,
});

// Use policy check in execute transaction
const transaction = await lazorkitClient.executeTxn({
  // ... other params
  policyInstruction: checkPolicyIx, // Or null to use default policy check
});
```

### Transaction Chunks (Deferred Execution)

For complex transactions that exceed transaction size limits, you can create chunks:

```typescript
// Create a chunk with multiple instructions
const chunkTx = await lazorkitClient.createChunkTxn({
  payer: payer.publicKey,
  smartWallet: smartWallet.publicKey,
  passkeySignature: {
    passkeyPublicKey: [/* 33 bytes */],
    signature64: 'base64-signature',
    clientDataJsonRaw64: 'base64-client-data',
    authenticatorDataRaw64: 'base64-auth-data',
  },
  credentialHash: [/* 32 bytes */],
  policyInstruction: null, // Optional: use default policy check if null
  cpiInstructions: [instruction1, instruction2, instruction3], // Multiple instructions
  timestamp: new BN(Math.floor(Date.now() / 1000)),
}, {
  computeUnitLimit: 300000, // Higher limit for complex transactions
  useVersionedTransaction: true,
});

// Execute chunk (no authentication needed - uses pre-authorized chunk)
const executeTx = await lazorkitClient.executeChunkTxn({
  payer: payer.publicKey,
  smartWallet: smartWallet.publicKey,
  cpiInstructions: [instruction1, instruction2, instruction3], // Same instructions as chunk
});

// Close chunk to refund rent (if not executed)
const closeTx = await lazorkitClient.closeChunkTxn({
  payer: payer.publicKey,
  smartWallet: smartWallet.publicKey,
  nonce: chunkNonce,
});
```

### Using the Default Policy Client

```typescript
// Get required PDAs
const walletStateData = await lazorkitClient.getWalletStateData(smartWallet);
const policySigner = lazorkitClient.getWalletDevicePubkey(smartWallet, credentialHash);
const walletState = lazorkitClient.getWalletStatePubkey(smartWallet);

// Build policy initialization instruction
const initPolicyIx = await defaultPolicyClient.buildInitPolicyIx({
  walletId: walletStateData.walletId,
  passkeyPublicKey: passkeyPublicKey,
  credentialHash: credentialHash,
  policySigner: policySigner,
  smartWallet: smartWallet,
  walletState: walletState,
});

// Build policy check instruction
const checkPolicyIx = await defaultPolicyClient.buildCheckPolicyIx({
  walletId: walletStateData.walletId,
  passkeyPublicKey: passkeyPublicKey,
  policySigner: policySigner,
  smartWallet: smartWallet,
  credentialHash: credentialHash,
  policyData: walletStateData.policyData,
});
```

## Testing

Run the test suite:

```bash
anchor test
```

The test suite includes:

- Smart wallet creation and initialization
- Default policy implementation
- Transaction execution
- Policy management
- Session functionality

## Key Features

### Security

- **Passkey Authentication**: Uses secp256r1 WebAuthn for secure authentication
- **Multi-Device Support**: Add multiple wallet_devices to a single wallet
- **Policy-Based Validation**: Customizable transaction validation policies

### Flexibility

- **Custom Policy Programs**: Implement your own policy programs or use the default
- **Chunk-Based Execution**: Execute complex multi-step transactions using deferred execution chunks
- **Modular Design**: Clean separation between wallet management and policy logic

### Developer Experience

- **TypeScript SDK**: Full TypeScript support with generated types
- **Anchor Integration**: Built with Anchor framework for easy development
- **Comprehensive Testing**: Extensive test coverage
- **Clean API**: Well-organized, intuitive API with clear separation of concerns

## Program IDs

| Program        | Devnet                                         | Mainnet                                        |
| -------------- | ---------------------------------------------- | ---------------------------------------------- |
| LazorKit       | `Gsuz7YcA5sbMGVRXT3xSYhJBessW4xFC4xYsihNCqMFh` | `Gsuz7YcA5sbMGVRXT3xSYhJBessW4xFC4xYsihNCqMFh` |
| Default Policy | `BiE9vSdz9MidUiyjVYsu3PG4C1fbPZ8CVPADA9jRfXw7` | `BiE9vSdz9MidUiyjVYsu3PG4C1fbPZ8CVPADA9jRfXw7` |

## Address Lookup Table

The system uses an address lookup table to optimize transaction size:

- **Address**: `7Pr3DG7tRPAjVb44gqbxTj1KstikAuVZY7YmXdotVjLA`

## Recent Updates

### Simplified Contract (Lite Version)

The contract has been streamlined for better efficiency and clarity:

- **Simplified Instructions**: Reduced from 9+ instructions to 5 core instructions
- **Removed Direct Policy Management**: Policy operations are now handled through policy programs directly
- **Cleaner API**: More focused client methods with clear responsibilities
- **Better Transaction Handling**: Improved chunk-based execution for complex transactions

#### Key Changes:

**LazorKit Program:**
- Removed: `update_policy`, `invoke_policy`, `add_policy_program`, `update_config`
- Kept: `create_smart_wallet`, `execute`, `create_chunk`, `execute_chunk`, `close_chunk`

**Default Policy Program:**
- Removed: `add_device`, `remove_device`, `destroy_policy`
- Kept: `init_policy`, `check_policy`

**Client Methods:**
- `createSmartWalletTxn()` - Create new smart wallet
- `executeTxn()` - Execute transaction with authentication
- `createChunkTxn()` - Create deferred execution chunk
- `executeChunkTxn()` - Execute chunk (no auth needed)
- `closeChunkTxn()` - Close chunk and refund rent

See the [sdk README](./sdk/README.md) for detailed API documentation and examples.

### SDK Refactor

The TypeScript integration SDK has been refactored to make contracts easier to use securely:

- **Centralized PDA Logic**: `client/internal/walletPdas.ts` now derives every PDA with shared validation, removing duplicated logic in `LazorkitClient`.
- **Policy Resolution Layer**: `client/internal/policyResolver.ts` automatically falls back to the default policy program when callers don't pass custom instructions, keeping execute/create flows concise.
- **CPI Utilities**: `client/internal/cpi.ts` provides reusable helpers that build split indices, CPI hashes, and remaining account metas, ensuring signer flags are preserved and CPI hashing stays consistent between `messages.ts` and runtime builders.
- **Validation Layer**: `validation.ts` provides comprehensive validation helpers with clear error messages, including `credentialHashFromBase64`, `byteArrayEquals`, and type-safe assertions.
- **Type Safety**: Full TypeScript support with generated Anchor types and comprehensive type definitions in `types.ts`.

These changes improve code organization, reduce duplication, enhance security, and make the SDK easier to maintain and extend.

## Contributing

1. Fork the repository
2. Create your feature branch (`git checkout -b feature/amazing-feature`)
3. Commit your changes (`git commit -m 'Add some amazing feature'`)
4. Push to the branch (`git push origin feature/amazing-feature`)
5. Open a Pull Request

---

Built and maintained by the [LazorKit](https://lazorkit.com/).

Licensed under MIT. See [LICENSE](LICENSE) for details.
