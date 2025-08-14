# LazorKit - Smart Wallet Management System

A comprehensive Solana-based smart wallet management system that provides secure passkey authentication, customizable rule engines, and flexible transaction execution capabilities.

## Overview

LazorKit is a sophisticated smart wallet system built on Solana that enables users to create and manage smart wallets with advanced security features:

- **Passkey Authentication**: Secure authentication using secp256r1 WebAuthn credentials
- **Rule Engine System**: Customizable transaction rules with a default rule implementation
- **Smart Wallet Management**: Create, configure, and manage smart wallets with multiple authenticators
- **CPI (Cross-Program Invocation) Support**: Execute complex transactions with committed state
- **Whitelist Management**: Control which rule programs can be used

## Architecture

### Programs

The system consists of two main Solana programs:

#### 1. LazorKit Program (`J6Big9w1VNeRZgDWH5qmNz2XFq5QeZbqC8caqSE5W`)
The core smart wallet program that handles:
- Smart wallet creation and initialization
- Passkey authentication management
- Rule program integration
- Transaction execution and CPI handling
- Configuration management

**Key Instructions:**
- `initialize` - Initialize the program
- `create_smart_wallet` - Create a new smart wallet with passkey
- `change_rule_direct` - Change wallet rules directly
- `call_rule_direct` - Execute rule program calls
- `execute_txn_direct` - Execute transactions directly
- `commit_cpi` - Commit CPI state for complex transactions
- `execute_committed` - Execute committed CPI transactions

#### 2. Default Rule Program (`CNT2aEgxucQjmt5SRsA6hSGrt241Bvc9zsgPvSuMjQTE`)
A reference implementation of transaction rules that provides:
- Rule initialization and validation
- Device management for multi-device wallets
- Transaction checking and approval logic

**Key Instructions:**
- `init_rule` - Initialize rule for a smart wallet
- `check_rule` - Validate transaction against rules
- `add_device` - Add new authenticator device

### Contract Integration SDK

The `contract-integration` folder provides a comprehensive TypeScript SDK for interacting with the LazorKit system:

```
contract-integration/
├── client/
│   ├── lazorkit.ts      # Main LazorKit client
│   └── defaultRule.ts   # Default rule client
├── anchor/
│   ├── idl/            # Anchor IDL definitions
│   └── types/          # Generated TypeScript types
├── pda/                # PDA derivation utilities
├── webauthn/           # WebAuthn/secp256r1 utilities
├── messages.ts         # Message building utilities
├── types.ts           # TypeScript type definitions
├── constants.ts       # Program constants
└── utils.ts           # Utility functions
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

# Deploy Default Rule program
anchor deploy --provider.cluster devnet --program-name default_rule
```

### Initialize IDL

```bash
# Initialize IDL for LazorKit
anchor idl init -f ./target/idl/lazorkit.json J6Big9w1VNeRZgDWH5qmNz2Nd6XFq5QeZbqC8caqSE5W

# Initialize IDL for Default Rule
anchor idl init -f ./target/idl/default_rule.json CNT2aEgxucQjmt5SRsA6hSGrt241Bvc9zsgPvSuMjQTE
```

## SDK Usage

### Basic Setup

```typescript
import { LazorkitClient, DefaultRuleClient } from './contract-integration';
import { Connection } from '@solana/web3.js';

// Initialize connection
const connection = new Connection('YOUR_RPC_URL');

// Create clients
const lazorkitClient = new LazorkitClient(connection);
const defaultRuleClient = new DefaultRuleClient(connection);
```

### Creating a Smart Wallet

```typescript
import { BN } from '@coral-xyz/anchor';

// Generate wallet ID
const walletId = lazorkitClient.generateWalletId();

// Create smart wallet with passkey
const createWalletTxn = await lazorkitClient.createSmartWalletTxn(
  passkeyPubkey,        // secp256r1 public key
  initRuleInstruction,  // Default rule initialization
  payer.publicKey
);
```

### Executing Transactions

```typescript
// Execute transaction directly
const executeTxn = await lazorkitClient.executeTxnDirectTxn(
  smartWallet,
  passkeyPubkey,
  signature,
  transactionInstruction,
  null // rule instruction (optional)
);

// Execute with rule validation
const executeWithRule = await lazorkitClient.executeTxnDirectTxn(
  smartWallet,
  passkeyPubkey,
  signature,
  transactionInstruction,
  ruleInstruction
);
```

### Managing Rules

```typescript
// Change wallet rules
const changeRuleTxn = await lazorkitClient.changeRuleDirectTxn(
  smartWallet,
  passkeyPubkey,
  signature,
  destroyRuleInstruction,
  initRuleInstruction,
  newPasskey
);

// Call rule program
const callRuleTxn = await lazorkitClient.callRuleDirectTxn(
  smartWallet,
  passkeyPubkey,
  signature,
  ruleInstruction,
  newPasskey
);
```

### CPI (Cross-Program Invocation)

```typescript
// Commit CPI state
const commitTxn = await lazorkitClient.commitCpiTxn(
  smartWallet,
  passkeyPubkey,
  signature,
  cpiInstruction,
  nonce
);

// Execute committed CPI
const executeCommittedTxn = await lazorkitClient.executeCommittedTxn(
  smartWallet,
  cpiData
);
```

## Testing

Run the test suite:

```bash
anchor test
```

The test suite includes:
- Smart wallet creation and initialization
- Default rule implementation
- Transaction execution
- Rule management
- CPI functionality

## Key Features

### Security
- **Passkey Authentication**: Uses secp256r1 WebAuthn for secure authentication
- **Multi-Device Support**: Add multiple authenticator devices to a single wallet
- **Rule-Based Validation**: Customizable transaction validation rules

### Flexibility
- **Custom Rule Programs**: Implement your own rule programs or use the default
- **CPI Support**: Execute complex multi-step transactions
- **Whitelist Management**: Control which rule programs can be used

### Developer Experience
- **TypeScript SDK**: Full TypeScript support with generated types
- **Anchor Integration**: Built with Anchor framework for easy development
- **Comprehensive Testing**: Extensive test coverage

## Program IDs

| Program | Devnet | Mainnet |
|---------|--------|---------|
| LazorKit | `J6Big9w1VNeRZgDWH5qmNz2Nd6XFq5QeZbqC8caqSE5W` | `J6Big9w1VNeRZgDWH5qmNz2Nd6XFq5QeZbqC8caqSE5W` |
| Default Rule | `CNT2aEgxucQjmt5SRsA6hSGrt241Bvc9zsgPvSuMjQTE` | `CNT2aEgxucQjmt5SRsA6hSGrt241Bvc9zsgPvSuMjQTE` |

## Address Lookup Table

The system uses an address lookup table to optimize transaction size:
- **Address**: `7Pr3DG7tRPAjVb44gqbxTj1KstikAuVZY7YmXdotVjLA`

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