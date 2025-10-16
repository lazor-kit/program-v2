# LazorKit Contract Integration

This directory contains the TypeScript integration code for the LazorKit smart wallet program. The code provides a clean, well-organized API with clear separation of concerns and comprehensive transaction building capabilities.

## 📁 Directory Structure

```
contract-integration/
├── anchor/           # Generated Anchor types and IDL
│   ├── idl/         # JSON IDL files
│   └── types/       # TypeScript type definitions
├── client/           # Main client classes
│   ├── lazorkit.ts  # Main LazorkitClient
│   └── defaultPolicy.ts # DefaultPolicyClient
├── pda/             # PDA derivation functions
│   ├── lazorkit.ts  # Lazorkit PDA functions
│   └── defaultPolicy.ts # Default policy PDA functions
├── webauthn/        # WebAuthn/Passkey utilities
│   └── secp256r1.ts # Secp256r1 signature verification
├── examples/        # Usage examples
├── auth.ts          # Authentication utilities
├── transaction.ts   # Transaction building utilities
├── utils.ts         # General utilities
├── messages.ts      # Message building utilities
├── types.ts         # TypeScript type definitions
├── index.ts         # Main exports
└── README.md        # This file
```

## 🚀 Quick Start

```typescript
import { LazorkitClient, DefaultPolicyClient } from './contract-integration';
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
const executeTx = await lazorkitClient.executeTxn({
  payer: payer.publicKey,
  smartWallet: smartWallet,
  passkeySignature: {
    passkeyPublicKey: [/* 33 bytes */],
    signature64: 'base64-signature',
    clientDataJsonRaw64: 'base64-client-data',
    authenticatorDataRaw64: 'base64-auth-data',
  },
  policyInstruction: null,
  cpiInstruction: transferInstruction,
  timestamp: new BN(Math.floor(Date.now() / 1000)),
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

- **PDA Derivation**: `getConfigPubkey()`, `getSmartWalletPubkey()`, `getWalletDevicePubkey()`, etc.
- **Account Data**: `getWalletStateData()`, `getWalletDeviceData()`, etc.
- **Wallet Search**: `getSmartWalletByPasskey()`, `getSmartWalletByCredentialHash()`, `findSmartWallet()`
- **Low-level Builders**: `buildCreateSmartWalletIns()`, `buildExecuteIns()`, etc.
- **High-level Transaction Builders**: 
  - `createSmartWalletTxn()` - Create new smart wallet
  - `executeTxn()` - Execute transaction with authentication
  - `callPolicyTxn()` - Call wallet policy
  - `changePolicyTxn()` - Change wallet policy
  - `createChunkTxn()` - Create deferred execution chunk
  - `executeChunkTxn()` - Execute deferred chunk

#### `DefaultPolicyClient`

Client for interacting with the default policy program.

### Authentication

The integration provides utilities for passkey authentication:

```typescript
import { buildPasskeyVerificationInstruction } from './contract-integration';

// Build verification instruction
const authInstruction = buildPasskeyVerificationInstruction({
  passkeyPubkey: [
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
} from './contract-integration';

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
  passkeyPubkey: number[];
  signature64: string;
  clientDataJsonRaw64: string;
  authenticatorDataRaw64: string;
}

// Smart Wallet Actions
enum SmartWalletAction {
  UpdatePolicy = 'update_policy',
  InvokePolicy = 'invoke_policy',
  ExecuteTransaction = 'execute_transaction',
}

// Action Arguments
type SmartWalletActionArgs = {
  type: SmartWalletAction;
  args: ArgsByAction[SmartWalletAction];
};

// Transaction Parameters
interface CreateSmartWalletParams {
  payer: PublicKey;
  passkeyPubkey: number[];
  credentialIdBase64: string;
  policyInstruction?: TransactionInstruction | null;
  isPayForUser?: boolean;
  smartWalletId?: BN;
}

interface ExecuteTransactionParams {
  payer: PublicKey;
  smartWallet: PublicKey;
  passkeySignature: PasskeySignature;
  policyInstruction: TransactionInstruction | null;
  cpiInstruction: TransactionInstruction;
}
```

## 🏗️ Architecture

### Separation of Concerns

1. **Authentication (`auth.ts`)**: Handles passkey signature verification
2. **Transaction Building (`transaction.ts`)**: Manages transaction construction
3. **Message Building (`messages.ts`)**: Creates authorization messages
4. **PDA Derivation (`pda/`)**: Handles program-derived address calculations
5. **Client Logic (`client/`)**: High-level business logic and API

### Method Categories

#### Low-Level Instruction Builders

Methods that build individual instructions:

- `buildCreateSmartWalletIns()`
- `buildExecuteIns()`
- `buildInvokePolicyInstruction()`
- `buildUpdatePolicyInstruction()`
- `buildCreateChunkInstruction()`
- `buildExecuteSessionTransactionInstruction()`

#### High-Level Transaction Builders

Methods that build complete transactions with authentication:

- `createSmartWalletTxn()`
- `executeTransactionWithAuth()`
- `invokePolicyWithAuth()`
- `updatePolicyWithAuth()`
- `createChunkWithAuth()`
- `executeSessionTransaction()`

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

### From Old API to New API

**Old:**

```typescript
await client.createSmartWalletTx({
  payer: payer.publicKey,
  passkeyPubkey: [
    /* bytes */
  ],
  credentialIdBase64: 'base64',
  ruleInstruction: null,
});
```

**New:**

```typescript
await client.createSmartWalletTxn({
  payer: payer.publicKey,
  passkeyPubkey: [
    /* bytes */
  ],
  credentialIdBase64: 'base64',
  policyInstruction: null,
});
```

### Key Changes

1. **Method Names**: More descriptive and consistent

   - `executeTxnDirectTx` → `executeTransactionWithAuth`
   - `callRuleDirectTx` → `invokePolicyWithAuth`
   - `changeRuleDirectTx` → `updatePolicyWithAuth`
   - `commitCpiTx` → `createChunkWithAuth`
   - `executeCommitedTx` → `executeSessionTransaction`

2. **Parameter Structure**: Better organized with typed interfaces

   - Authentication data grouped in `PasskeySignature` for methods that require signatures
   - Clear separation of required vs optional parameters
   - Consistent naming: `policyInstruction` instead of `ruleInstruction`

3. **Return Types**: More consistent and informative

   - All high-level methods return `VersionedTransaction`
   - Legacy methods return `Transaction` for backward compatibility

4. **Type Names**: More accurate and generic

   - `MessageArgs` → `SmartWalletActionArgs` (can be used anywhere, not just messages)

5. **Client Names**: Updated for consistency

   - `DefaultRuleClient` → `DefaultPolicyClient`

6. **Terminology**: All "rule" references changed to "policy"
   - `ruleInstruction` → `policyInstruction`
   - `ruleData` → `policyData`
   - `checkRule` → `checkPolicy`
   - `initRule` → `initPolicy`

## 🧪 Testing

The integration includes comprehensive type safety and can be tested with:

```typescript
// Test smart wallet creation
it('should create smart wallet successfully', async () => {
  const { transaction, smartWalletId, smartWallet } =
    await client.createSmartWalletTxn({
      payer: payer.publicKey,
      passkeyPubkey: [
        /* test bytes */
      ],
      credentialIdBase64: 'test-credential',
      isPayForUser: true,
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
    passkeyPubkey: [
      /* 33 bytes */
    ],
    credentialIdBase64: 'base64-credential',
    isPayForUser: true,
  });
```

### Executing a Transaction with Authentication

```typescript
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
  policyInstruction: null,
  cpiInstruction: transferInstruction,
  timestamp: new BN(Math.floor(Date.now() / 1000)),
}, {
  computeUnitLimit: 200000, // Set compute unit limit
  useVersionedTransaction: true
});
```

### Creating a Transaction Session

```typescript
const sessionTx = await client.createChunkTxn({
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
  policyInstruction: null,
  cpiInstructions: [transferInstruction1, transferInstruction2],
  timestamp: new BN(Math.floor(Date.now() / 1000)),
}, {
  computeUnitLimit: 300000, // Higher limit for multiple instructions
  useVersionedTransaction: true
});
```

### Building Authorization Messages

```typescript
const message = await client.buildAuthorizationMessage({
  action: {
    type: SmartWalletAction.ExecuteTransaction,
    args: {
      policyInstruction: null,
      cpiInstruction: transferInstruction,
    },
  },
  payer: payer.publicKey,
  smartWallet: smartWallet.publicKey,
  passkeyPubkey: [
    /* 33 bytes */
  ],
});
```

### Using the Default Policy Client

```typescript
import { DefaultPolicyClient } from './contract-integration';

const defaultPolicyClient = new DefaultPolicyClient(connection);

// Build policy initialization instruction
const initPolicyIx = await defaultPolicyClient.buildInitPolicyIx(
  payer.publicKey,
  smartWallet.publicKey,
  walletDevice.publicKey
);

// Build policy check instruction
const checkPolicyIx = await defaultPolicyClient.buildCheckPolicyIx(
  walletDevice.publicKey
);

// Build add device instruction
const addDeviceIx = await defaultPolicyClient.buildAddDeviceIx(
  payer.publicKey,
  walletDevice.publicKey,
  newWalletDevice.publicKey
);
```

See the `tests/` directory for comprehensive usage examples of all the new API methods.
