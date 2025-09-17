# LazorKit - Smart Wallet Management System

A comprehensive Solana-based smart wallet management system that provides secure passkey authentication, customizable policy engines, and flexible transaction execution capabilities.

## Overview

LazorKit is a sophisticated smart wallet system built on Solana that enables users to create and manage smart wallets with advanced security features:

- **Passkey Authentication**: Secure authentication using secp256r1 WebAuthn credentials
- **Policy Engine System**: Customizable transaction policies with a default policy implementation
- **Smart Wallet Management**: Create, configure, and manage smart wallets with multiple wallet_devices
- **Transaction Session Support**: Execute complex transactions with session-based state management
- **Policy Registry Management**: Control which policy programs can be used

## Architecture

### Programs

The system consists of two main Solana programs:

#### 1. LazorKit Program (`J6Big9w1VNeRZgDWH5qmNz2XFq5QeZbqC8caqSE5W`)

The core smart wallet program that handles:

- Smart wallet creation and initialization
- Passkey authentication management
- Policy program integration
- Transaction execution and session handling
- Configuration management

**Key Instructions:**

- `initialize` - Initialize the program
- `create_smart_wallet` - Create a new smart wallet with passkey
- `update_policy` - Update wallet policies directly
- `invoke_policy` - Execute policy program calls
- `execute_transaction` - Execute transactions directly
- `create_transaction_session` - Create session for complex transactions
- `execute_session_transaction` - Execute session-based transactions
- `add_policy_program` - Add programs to the policy registry
- `update_config` - Update program configuration

#### 2. Default Policy Program (`CNT2aEgxucQjmt5SRsA6hSGrt241Bvc9zsgPvSuMjQTE`)

A reference implementation of transaction policies that provides:

- Policy initialization and validation
- Device management for multi-device wallets
- Transaction checking and approval logic

**Key Instructions:**

- `init_policy` - Initialize policy for a smart wallet
- `check_policy` - Validate transaction against policies
- `add_device` - Add new wallet_device

### Contract Integration SDK

The `contract-integration` folder provides a comprehensive TypeScript SDK for interacting with the LazorKit system:

```
contract-integration/
├── anchor/           # Generated Anchor types and IDL
├── client/           # Main client classes
├── pda/             # PDA derivation functions
├── webauthn/        # WebAuthn/Passkey utilities
├── auth.ts          # Authentication utilities
├── transaction.ts   # Transaction building utilities
├── utils.ts         # General utilities
├── messages.ts      # Message building utilities
├── constants.ts     # Program constants
├── types.ts         # TypeScript type definitions
├── index.ts         # Main exports
└── README.md        # This file
```

## Installation

### Prerequisites

- Node.js (v16 or higher)
- Solana CLI
- Anchor Framework (v0.31.0)
- Rust (for program development)

### Setup

1. Clone the repository:

```bash
git clone <repository-url>
cd wallet-management-contract
```

2. Install dependencies:

```bash
npm install
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
anchor idl init -f ./target/idl/lazorkit.json J6Big9w1VNeRZgDWH5qmNz2Nd6XFq5QeZbqC8caqSE5W

# Initialize IDL for Default Policy
anchor idl init -f ./target/idl/default_policy.json CNT2aEgxucQjmt5SRsA6hSGrt241Bvc9zsgPvSuMjQTE
```

### Upgrade IDL

```bash
# Initialize IDL for LazorKit
anchor idl upgrade J6Big9w1VNeRZgDWH5qmNz2Nd6XFq5QeZbqC8caqSE5W -f ./target/idl/lazorkit.json

# Initialize IDL for Default Policy
anchor idl upgrade CNT2aEgxucQjmt5SRsA6hSGrt241Bvc9zsgPvSuMjQTE -f ./target/idl/default_policy.json
```

## SDK Usage

### Basic Setup

```typescript
import { LazorkitClient, DefaultPolicyClient } from './contract-integration';
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

// Generate wallet ID
const walletId = lazorkitClient.generateWalletId();

// Create smart wallet with passkey
const { transaction, smartWalletId, smartWallet } =
  await lazorkitClient.createSmartWalletTxn({
    payer: payer.publicKey,
    passkeyPubkey: [
      /* 33 bytes */
    ],
    credentialIdBase64: 'base64-credential',
    isPayForUser: true,
  });
```

### Executing Transactions

```typescript
// Execute transaction with authentication
const transaction = await lazorkitClient.executeTransactionWithAuth({
  payer: payer.publicKey,
  smartWallet: smartWallet.publicKey,
  passkeySignature: {
    passkeyPubkey: [
      /* 33 bytes */
    ],
    signature64: 'base64-signature',
    clientDataJsonRaw64: 'base64-client-data',
    authenticatorDataRaw64: 'base64-auth-data',
  },
  policyInstruction: null,
  cpiInstruction: transferInstruction,
});
```

### Managing Policies

```typescript
// Update wallet policies
const updateTx = await lazorkitClient.updatePolicyWithAuth({
  payer: payer.publicKey,
  smartWallet: smartWallet.publicKey,
  passkeySignature: {
    passkeyPubkey: [
      /* 33 bytes */
    ],
    signature64: 'base64-signature',
    clientDataJsonRaw64: 'base64-client-data',
    authenticatorDataRaw64: 'base64-auth-data',
  },
  destroyPolicyInstruction: destroyInstruction,
  initPolicyInstruction: initInstruction,
  newWalletDevice: {
    passkeyPubkey: [
      /* 33 bytes */
    ],
    credentialIdBase64: 'base64-credential',
  },
});

// Invoke policy program
const invokeTx = await lazorkitClient.invokePolicyWithAuth({
  payer: payer.publicKey,
  smartWallet: smartWallet.publicKey,
  passkeySignature: {
    passkeyPubkey: [
      /* 33 bytes */
    ],
    signature64: 'base64-signature',
    clientDataJsonRaw64: 'base64-client-data',
    authenticatorDataRaw64: 'base64-auth-data',
  },
  policyInstruction: policyInstruction,
  newWalletDevice: null,
});
```

### Transaction Sessions

```typescript
// Create transaction session
const sessionTx = await lazorkitClient.createChunkWithAuth({
  payer: payer.publicKey,
  smartWallet: smartWallet.publicKey,
  passkeySignature: {
    passkeyPubkey: [
      /* 33 bytes */
    ],
    signature64: 'base64-signature',
    clientDataJsonRaw64: 'base64-client-data',
    authenticatorDataRaw64: 'base64-auth-data',
  },
  policyInstruction: null,
  expiresAt: Math.floor(Date.now() / 1000) + 3600, // 1 hour
});

// Execute session transaction (no authentication needed)
const executeTx = await lazorkitClient.executeSessionTransaction({
  payer: payer.publicKey,
  smartWallet: smartWallet.publicKey,
  cpiInstruction: complexInstruction,
});
```

### Using the Default Policy Client

```typescript
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
- **Session Support**: Execute complex multi-step transactions with session management
- **Policy Registry Management**: Control which policy programs can be used

### Developer Experience

- **TypeScript SDK**: Full TypeScript support with generated types
- **Anchor Integration**: Built with Anchor framework for easy development
- **Comprehensive Testing**: Extensive test coverage
- **Clean API**: Well-organized, intuitive API with clear separation of concerns

## Program IDs

| Program        | Devnet                                         | Mainnet                                        |
| -------------- | ---------------------------------------------- | ---------------------------------------------- |
| LazorKit       | `J6Big9w1VNeRZgDWH5qmNz2Nd6XFq5QeZbqC8caqSE5W` | `J6Big9w1VNeRZgDWH5qmNz2Nd6XFq5QeZbqC8caqSE5W` |
| Default Policy | `CNT2aEgxucQjmt5SRsA6hSGrt241Bvc9zsgPvSuMjQTE` | `CNT2aEgxucQjmt5SRsA6hSGrt241Bvc9zsgPvSuMjQTE` |

## Address Lookup Table

The system uses an address lookup table to optimize transaction size:

- **Address**: `7Pr3DG7tRPAjVb44gqbxTj1KstikAuVZY7YmXdotVjLA`

## Recent Updates

### Refactored API (v2.0)

The SDK has been completely refactored with:

- **Better Naming**: More descriptive and consistent method names
- **Improved Organization**: Clear separation of concerns with dedicated utility modules
- **Enhanced Type Safety**: Comprehensive TypeScript interfaces and type definitions
- **Cleaner Architecture**: Modular design with authentication, transaction building, and message utilities

#### Key Changes:

- `executeTxnDirectTx` → `executeTransactionWithAuth`
- `callRuleDirectTx` → `invokePolicyWithAuth`
- `changeRuleDirectTx` → `updatePolicyWithAuth`
- `commitCpiTx` → `createChunkWithAuth`
- `executeCommitedTx` → `executeSessionTransaction`
- `MessageArgs` → `SmartWalletActionArgs`
- `DefaultRuleClient` → `DefaultPolicyClient`
- All "rule" terminology changed to "policy" for consistency

See the [contract-integration README](./contract-integration/README.md) for detailed migration guide and examples.

## Contributing

1. Fork the repository
2. Create your feature branch (`git checkout -b feature/amazing-feature`)
3. Commit your changes (`git commit -m 'Add some amazing feature'`)
4. Push to the branch (`git push origin feature/amazing-feature`)
5. Open a Pull Request

## License

[Add your license information here]

## Support

For questions and support, please open an issue on GitHub or contact the development team.
