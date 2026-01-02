# LazorKit typescript sdk

This directory contains the TypeScript integration code for the LazorKit smart wallet program. The code provides a clean, well-organized API with clear separation of concerns and comprehensive transaction building capabilities.

## 📁 Directory Structure

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
└── README.md        # This file
```

## 🚀 Quick Start

```typescript
import { LazorkitClient, DefaultPolicyClient } from './sdk';
import { Connection } from '@solana/web3.js';

// Initialize clients
const connection = new Connection('https://api.mainnet-beta.solana.com');
const lazorkitClient = new LazorkitClient(connection);
const defaultPolicyClient = new DefaultPolicyClient(connection);

// Create a smart wallet
const { transaction, smartWalletId, smartWallet } =
  await lazorkitClient.createSmartWalletTxn({
    payer: payer.publicKey,
    passkeyPublicKey: [
      /* 33 bytes */
    ],
    credentialIdBase64: 'base64-credential-id',
    amount: new BN(0.01 * LAMPORTS_PER_SOL),
  });

// Execute a transaction with compute unit limit
const walletStateData = await lazorkitClient.getWalletStateData(smartWallet);
const executeTx = await lazorkitClient.executeTxn({
  payer: payer.publicKey,
  smartWallet: smartWallet,
  passkeySignature: {
    passkeyPublicKey: [/* 33 bytes */],
    signature64: 'base64-signature',
    clientDataJsonRaw64: 'base64-client-data',
    authenticatorDataRaw64: 'base64-auth-data',
  },
  credentialHash: [/* 32 bytes */],
  policyInstruction: null, // Use default policy check if null
  cpiInstruction: transferInstruction,
  timestamp: new BN(Math.floor(Date.now() / 1000)),
  smartWalletId: walletStateData.walletId,
}, {
  computeUnitLimit: 200000, // Set compute unit limit
  useVersionedTransaction: true
});
```

## 📚 API Overview

### Client Classes

#### `LazorkitClient`

The main client for interacting with the LazorKit program.

**Key Methods:**

- **PDA Derivation**: `getSmartWalletPubkey()`, `getWalletStatePubkey()`, `getWalletAuthorityPubkey()`, `getChunkPubkey()`
- **Account Data**: `getWalletStateData()`, `getChunkData()`
- **Wallet Search**: `getSmartWalletByPasskey()`, `getSmartWalletByCredentialHash()`, `findSmartWallet()`
- **Low-level Builders**: `buildCreateSmartWalletIns()`, `buildExecuteIns()`, `buildCreateChunkIns()`, `buildExecuteChunkIns()`, `buildCloseChunkIns()`
- **High-level Transaction Builders**: 
  - `createSmartWalletTxn()` - Create new smart wallet
  - `executeTxn()` - Execute transaction with authentication
  - `createChunkTxn()` - Create deferred execution chunk (with authentication)
  - `executeChunkTxn()` - Execute deferred chunk (no authentication needed)
  - `closeChunkTxn()` - Close chunk and refund rent (no authentication needed)

#### `DefaultPolicyClient`

Client for interacting with the default policy program.

**Key Methods:**

- **PDA Derivation**: `policyPda()` - Get policy PDA for a smart wallet
- **Policy Data**: `getPolicyDataSize()` - Get default policy data size
- **Instruction Builders**:
  - `buildInitPolicyIx()` - Build policy initialization instruction
  - `buildCheckPolicyIx()` - Build policy check instruction

### Authentication

The integration provides utilities for passkey authentication:

```typescript
import { buildPasskeyVerificationInstruction } from './sdk';

// Build verification instruction
const authInstruction = buildPasskeyVerificationInstruction({
  passkeyPublicKey: [
    /* 33 bytes */
  ],
  signature64: 'base64-signature',
  clientDataJsonRaw64: 'base64-client-data',
  authenticatorDataRaw64: 'base64-auth-data',
});
```

### Transaction Building

Utilities for building different types of transactions:

```typescript
import {
  buildVersionedTransaction,
  buildLegacyTransaction,
  buildTransaction,
} from './sdk';

// Build versioned transaction (v0)
const v0Tx = await buildVersionedTransaction(connection, payer, instructions);

// Build legacy transaction
const legacyTx = await buildLegacyTransaction(connection, payer, instructions);

// Build transaction with compute unit limit
const txWithCULimit = await buildTransaction(connection, payer, instructions, {
  computeUnitLimit: 200000, // Set compute unit limit to 200,000
  useVersionedTransaction: true
});
```

#### Transaction Builder Options

The `TransactionBuilderOptions` interface supports the following options:

```typescript
interface TransactionBuilderOptions {
  useVersionedTransaction?: boolean;           // Use versioned transaction (v0)
  addressLookupTable?: AddressLookupTableAccount; // Address lookup table for v0
  recentBlockhash?: string;                    // Custom recent blockhash
  computeUnitLimit?: number;                   // Set compute unit limit
}
```

**Compute Unit Limit**: When specified, a `setComputeUnitLimit` instruction will be automatically prepended to your transaction. This is useful for complex transactions that might exceed the default compute unit limit.

**Important Note**: When using compute unit limits, the `verifyInstructionIndex` in all smart wallet instructions is automatically adjusted. This is because the CU limit instruction is prepended at index 0, shifting the authentication instruction to index 1.

## ⚡ Compute Unit Limit Management

The contract integration automatically handles compute unit limits and instruction indexing:

### Automatic Index Adjustment

When you specify a `computeUnitLimit`, the system automatically:
1. Prepends a `setComputeUnitLimit` instruction at index 0
2. Adjusts all `verifyInstructionIndex` values from 0 to 1
3. Maintains proper instruction ordering

### Usage Examples

```typescript
// Without compute unit limit
const tx1 = await client.executeTxn(params, {
  useVersionedTransaction: true
});
// verifyInstructionIndex = 0

// With compute unit limit
const tx2 = await client.executeTxn(params, {
  computeUnitLimit: 200000,
  useVersionedTransaction: true
});
// verifyInstructionIndex = 1 (automatically adjusted)
```

### Recommended CU Limits

- **Simple transfers**: 50,000 - 100,000
- **Token operations**: 100,000 - 150,000
- **Complex transactions**: 200,000 - 300,000
- **Multiple operations**: 300,000+

## 🔧 Type Definitions

### Core Types

```typescript
// Authentication
interface PasskeySignature {
  passkeyPublicKey: number[];
  signature64: string;
  clientDataJsonRaw64: string;
  authenticatorDataRaw64: string;
}

// Smart Wallet Actions
enum SmartWalletAction {
  Execute = 'execute',
  CreateChunk = 'create_chunk',
  ExecuteChunk = 'execute_chunk',
}

// Action Arguments
type SmartWalletActionArgs<K extends SmartWalletAction = SmartWalletAction> = {
  type: K;
  args: ArgsByAction[K];
};

// Transaction Parameters
interface CreateSmartWalletParams {
  payer: PublicKey;
  passkeyPublicKey: number[];
  credentialIdBase64: string;
  amount?: BN;
  policyInstruction?: TransactionInstruction | null;
  smartWalletId?: BN;
  policyDataSize?: number;
}

interface ExecuteParams {
  payer: PublicKey;
  smartWallet: PublicKey;
  passkeySignature: PasskeySignature;
  credentialHash: number[];
  policyInstruction: TransactionInstruction | null;
  cpiInstruction: TransactionInstruction;
  timestamp: BN;
  smartWalletId: BN;
  cpiSigners?: readonly PublicKey[]; // Optional: signers for CPI instruction
}

interface CreateChunkParams {
  payer: PublicKey;
  smartWallet: PublicKey;
  passkeySignature: PasskeySignature;
  credentialHash: number[];
  policyInstruction: TransactionInstruction | null;
  cpiInstructions: TransactionInstruction[];
  timestamp: BN;
  cpiSigners?: readonly PublicKey[]; // Optional: signers for CPI instructions
}

interface ExecuteChunkParams {
  payer: PublicKey;
  smartWallet: PublicKey;
  cpiInstructions: TransactionInstruction[];
  cpiSigners?: readonly PublicKey[]; // Optional: signers for CPI instructions
}

interface CloseChunkParams {
  payer: PublicKey;
  smartWallet: PublicKey;
  nonce: BN;
}
```

## 🏗️ Architecture

### Separation of Concerns

1. **Authentication (`auth.ts`)**: Handles passkey signature verification
2. **Transaction Building (`transaction.ts`)**: Manages transaction construction
3. **Message Building (`messages.ts`)**: Creates authorization messages
4. **PDA Derivation (`pda/` and `client/internal/walletPdas.ts`)**: Handles program-derived address calculations
5. **Validation (`validation.ts`)**: Provides comprehensive validation helpers
6. **Policy Resolution (`client/internal/policyResolver.ts`)**: Automatically resolves policy instructions
7. **CPI Utilities (`client/internal/cpi.ts`)**: Handles CPI instruction building and account management
8. **Client Logic (`client/`)**: High-level business logic and API

### Method Categories

#### Low-Level Instruction Builders

Methods that build individual instructions:

- `buildCreateSmartWalletIns()` - Build create smart wallet instruction
- `buildExecuteIns()` - Build execute instruction
- `buildCreateChunkIns()` - Build create chunk instruction
- `buildExecuteChunkIns()` - Build execute chunk instruction
- `buildCloseChunkIns()` - Build close chunk instruction

#### High-Level Transaction Builders

Methods that build complete transactions:

- `createSmartWalletTxn()` - Create new smart wallet (with optional policy initialization)
- `executeTxn()` - Execute transaction with authentication
- `createChunkTxn()` - Create deferred execution chunk (with authentication)
- `executeChunkTxn()` - Execute chunk (no authentication needed)
- `closeChunkTxn()` - Close chunk and refund rent (no authentication needed)

#### Utility Methods

Helper methods for common operations:

- `generateWalletId()`
- `getWalletStateData()`
- `buildAuthorizationMessage()`
- `getSmartWalletByPasskey()`
- `getSmartWalletByCredentialHash()`
- `findSmartWallet()`

## 🔍 Wallet Search Functionality

The LazorKit client provides powerful search capabilities to find smart wallets using only passkey public keys or credential hashes. This solves the common problem of not knowing the smart wallet address when you only have authentication credentials.

### Search Methods

#### `getSmartWalletByPasskey(passkeyPublicKey: number[])`

Finds a smart wallet by searching through all WalletState accounts for one containing the specified passkey public key.

```typescript
const result = await lazorkitClient.getSmartWalletByPasskey(passkeyPublicKey);
if (result.smartWallet) {
  console.log('Found wallet:', result.smartWallet.toString());
  console.log('Wallet state:', result.walletState.toString());
  console.log('Device slot:', result.deviceSlot);
}
```

#### `getSmartWalletByCredentialHash(credentialHash: number[])`

Finds a smart wallet by searching through all WalletState accounts for one containing the specified credential hash.

```typescript
const result = await lazorkitClient.getSmartWalletByCredentialHash(credentialHash);
if (result.smartWallet) {
  console.log('Found wallet:', result.smartWallet.toString());
}
```

#### `findSmartWallet(passkeyPublicKey?: number[], credentialHash?: number[])`

Convenience method that tries both passkey and credential hash search approaches.

```typescript
const result = await lazorkitClient.findSmartWallet(passkeyPublicKey, credentialHash);
if (result.smartWallet) {
  console.log('Found wallet:', result.smartWallet.toString());
  console.log('Found by:', result.foundBy); // 'passkey' | 'credential'
}
```

### Return Types

All search methods return an object with:

```typescript
{
  smartWallet: PublicKey | null;           // The smart wallet address
  walletState: PublicKey | null;           // The wallet state PDA address  
  deviceSlot: {                            // The matching device information
    passkeyPubkey: number[];
    credentialHash: number[];
  } | null;
  foundBy?: 'passkey' | 'credential' | null; // How the wallet was found (findSmartWallet only)
}
```

### Performance Considerations

- **Efficiency**: These methods scan all WalletState accounts on-chain, so performance depends on the total number of wallets
- **Caching**: Consider caching results for frequently accessed wallets
- **Error Handling**: Methods gracefully handle corrupted or invalid account data

### Example Usage

```typescript
// Find wallet by passkey
const walletByPasskey = await lazorkitClient.getSmartWalletByPasskey(passkeyBytes);
if (walletByPasskey.smartWallet) {
  // Execute transaction with found wallet
  const tx = await lazorkitClient.executeTxn({
    smartWallet: walletByPasskey.smartWallet,
    passkeySignature: signature,
    // ... other params
  });
}

// Find wallet by credential hash
const walletByCredential = await lazorkitClient.getSmartWalletByCredentialHash(credentialHashBytes);

// Try both approaches
const wallet = await lazorkitClient.findSmartWallet(passkeyBytes, credentialHashBytes);
```

## 🔄 Migration Guide

### Simplified Contract (Lite Version)

The contract has been streamlined to focus on core functionality:

#### Removed Methods

The following methods have been removed as part of the contract simplification:

- `invokePolicyWithAuth()` / `callPolicyTxn()` - Policy invocation is now handled through policy programs directly
- `updatePolicyWithAuth()` / `changePolicyTxn()` - Policy updates are handled through policy programs directly
- `buildInvokePolicyInstruction()` - No longer needed
- `buildUpdatePolicyInstruction()` - No longer needed

#### Updated Method Signatures

**Create Smart Wallet:**
```typescript
// Old (if existed)
await client.createSmartWalletTxn({
  payer: payer.publicKey,
  passkeyPubkey: [...], // old name
  credentialIdBase64: 'base64',
  isPayForUser: true, // old parameter
});

// New
await client.createSmartWalletTxn({
  payer: payer.publicKey,
  passkeyPublicKey: [...], // updated name
  credentialIdBase64: 'base64',
  amount: new BN(0.01 * 1e9), // new parameter
  policyInstruction: null, // optional policy init
});
```

**Execute Transaction:**
```typescript
// New - requires additional parameters
const walletStateData = await client.getWalletStateData(smartWallet);
await client.executeTxn({
  payer: payer.publicKey,
  smartWallet: smartWallet,
  passkeySignature: {
    passkeyPublicKey: [...], // updated name
    signature64: 'base64-signature',
    clientDataJsonRaw64: 'base64-client-data',
    authenticatorDataRaw64: 'base64-auth-data',
  },
  credentialHash: [...], // required
  policyInstruction: null,
  cpiInstruction: transferInstruction,
  timestamp: new BN(Math.floor(Date.now() / 1000)), // required
  smartWalletId: walletStateData.walletId, // required
});
```

**Chunk Methods:**
```typescript
// Old names
await client.createChunkWithAuth(...);
await client.executeSessionTransaction(...);

// New names
await client.createChunkTxn(...);
await client.executeChunkTxn(...);
await client.closeChunkTxn(...); // new method
```

#### Key Changes

1. **Simplified API**: Removed direct policy management methods
   - Policy operations are now handled through policy programs directly
   - Cleaner separation of concerns

2. **Parameter Updates**: 
   - `passkeyPubkey` → `passkeyPublicKey` (consistent naming)
   - `isPayForUser` → `amount` (more explicit)
   - Added required parameters: `credentialHash`, `timestamp`, `smartWalletId` for execute

3. **Chunk Naming**: More consistent chunk-related method names
   - `createChunkWithAuth` → `createChunkTxn`
   - `executeSessionTransaction` → `executeChunkTxn`
   - Added `closeChunkTxn` for closing unused chunks

4. **Default Policy Client**: Simplified to only essential methods
   - Removed: `buildAddDeviceIx()`, `buildRemoveDeviceIx()`, `buildDestroyPolicyIx()`
   - Kept: `buildInitPolicyIx()`, `buildCheckPolicyIx()`

## 🧪 Testing

The integration includes comprehensive type safety and can be tested with:

```typescript
// Test smart wallet creation
it('should create smart wallet successfully', async () => {
  const { transaction, smartWalletId, smartWallet } =
    await client.createSmartWalletTxn({
      payer: payer.publicKey,
      passkeyPublicKey: [
        /* test bytes */
      ],
      credentialIdBase64: 'test-credential',
      amount: new BN(0.01 * 1e9),
    });

  expect(smartWalletId).to.be.instanceOf(BN);
  expect(transaction).to.be.instanceOf(Transaction);
});
```

## 🔒 Security

- All authentication methods use proper passkey signature verification
- Transaction building includes proper instruction ordering
- PDA derivation follows secure patterns
- Type safety prevents common programming errors

## 📖 Examples

### Creating a Smart Wallet

```typescript
const { transaction, smartWalletId, smartWallet } =
  await client.createSmartWalletTxn({
    payer: payer.publicKey,
    passkeyPublicKey: [
      /* 33 bytes */
    ],
    credentialIdBase64: 'base64-credential',
    amount: new BN(0.01 * 1e9), // Optional: initial funding in lamports
    policyInstruction: null, // Optional: policy initialization instruction
  });
```

### Executing a Transaction with Authentication

```typescript
// First, get wallet state data
const walletStateData = await client.getWalletStateData(smartWallet);

const transaction = await client.executeTxn({
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
  credentialHash: [/* 32 bytes */], // Required
  policyInstruction: null, // Use default policy check if null
  cpiInstruction: transferInstruction,
  timestamp: new BN(Math.floor(Date.now() / 1000)), // Required
  smartWalletId: walletStateData.walletId, // Required
}, {
  computeUnitLimit: 200000, // Set compute unit limit
  useVersionedTransaction: true
});
```

### Creating a Transaction Chunk

```typescript
const chunkTx = await client.createChunkTxn({
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
  credentialHash: [/* 32 bytes */], // Required
  policyInstruction: null, // Use default policy check if null
  cpiInstructions: [transferInstruction1, transferInstruction2], // Multiple instructions
  timestamp: new BN(Math.floor(Date.now() / 1000)), // Required
}, {
  computeUnitLimit: 300000, // Higher limit for multiple instructions
  useVersionedTransaction: true
});

// Execute chunk (no authentication needed)
const executeTx = await client.executeChunkTxn({
  payer: payer.publicKey,
  smartWallet: smartWallet.publicKey,
  cpiInstructions: [transferInstruction1, transferInstruction2], // Same instructions as chunk
});

// Close chunk to refund rent (if not executed)
const closeTx = await client.closeChunkTxn({
  payer: payer.publicKey,
  smartWallet: smartWallet.publicKey,
  nonce: chunkNonce,
});
```

### Building Authorization Messages

```typescript
const message = await client.buildAuthorizationMessage({
  action: {
    type: SmartWalletAction.Execute,
    args: {
      policyInstruction: null,
      cpiInstruction: transferInstruction,
    },
  },
  payer: payer.publicKey,
  smartWallet: smartWallet.publicKey,
  passkeyPublicKey: [
    /* 33 bytes */
  ],
  credentialHash: [/* 32 bytes */],
  timestamp: new BN(Math.floor(Date.now() / 1000)),
});
```

### Using the Default Policy Client

```typescript
import { DefaultPolicyClient } from './sdk';

const defaultPolicyClient = new DefaultPolicyClient(connection);

// Get required PDAs
const walletStateData = await lazorkitClient.getWalletStateData(smartWallet);
const policySigner = lazorkitClient.getWalletAuthorityPubkey(smartWallet, credentialHash);
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

// Use policy instructions in transactions
const createWalletTx = await lazorkitClient.createSmartWalletTxn({
  payer: payer.publicKey,
  passkeyPublicKey: [...],
  credentialIdBase64: 'base64-credential',
  policyInstruction: initPolicyIx, // Initialize policy during wallet creation
});

const executeTx = await lazorkitClient.executeTxn({
  // ... other params
  policyInstruction: checkPolicyIx, // Or null to use default policy check
});
```

See the `tests/` directory for comprehensive usage examples of all the new API methods.
