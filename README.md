# Wallet Management Contract

A Solana-based smart wallet management system that provides secure and flexible wallet management capabilities with customizable rules and transfer limits.

## Overview

This project implements a smart wallet system on Solana with the following key features:
- Smart wallet creation and management
- Default rule implementation
- Transfer limit controls
- Whitelist rule program support
- Secp256r1 authentication

## Project Structure

```
├── programs/
│   ├── lazorkit/         # Main smart wallet program
│   ├── default_rule/     # Default rule implementation
├── sdk/
│   ├── lazor-kit.ts      # Main SDK implementation
│   ├── default-rule-program.ts
│   ├── utils.ts
│   ├── types.ts
│   └── constants.ts
└── tests/
    ├── smart_wallet_with_default_rule.test.ts
    ├── change_rule.test.ts
    ├── utils.ts
    └── constants.ts
```

## Prerequisites

- Node.js
- Solana CLI
- Anchor Framework
- Rust (for program development)

## Installation

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

## Program IDs

- LazorKit Program: `J6Big9w1VNeRZgDWH5qmNz2Nd6XFq5QeZbqC8caqSE5W`
- Transfer Limit Program: `34eqBPLfEvFGRNDbvpZLaa791J1e1zKMcFoVp19szLjY`
- Default Rule Program: `CNT2aEgxucQjmt5SRsA6hSGrt241Bvc9zsgPvSuMjQTE`

## Deployment

To deploy the programs and initialize the IDL:

```bash
# Initialize IDL for LazorKit
anchor idl init -f ./target/idl/lazorkit.json J6Big9w1VNeRZgDWH5qmNz2Nd6XFq5QeZbqC8caqSE5W

# Initialize IDL for Default Rule
anchor idl init -f ./target/idl/default_rule.json CNT2aEgxucQjmt5SRsA6hSGrt241Bvc9zsgPvSuMjQTE

# Upgrade IDL for LazorKit
anchor idl upgrade J6Big9w1VNeRZgDWH5qmNz2Nd6XFq5QeZbqC8caqSE5W -f ./target/idl/lazorkit.json

# Upgrade IDL for Default Rule
anchor idl upgrade CNT2aEgxucQjmt5SRsA6hSGrt241Bvc9zsgPvSuMjQTE -f ./target/idl/default_rule.json
```

## Testing

Run the test suite:

```bash
anchor test
```

The test suite includes:
- Smart wallet creation and initialization
- Default rule implementation
- Transfer limit functionality
- Rule change operations

## SDK Usage

The SDK provides a comprehensive interface for interacting with the smart wallet system:

```typescript
import { LazorKitProgram } from './sdk/lazor-kit';
import { DefaultRuleProgram } from './sdk/default-rule-program';

// Initialize the programs
const connection = new anchor.web3.Connection('YOUR_RPC_URL');
const lazorkitProgram = new LazorKitProgram(connection);
const defaultRuleProgram = new DefaultRuleProgram(connection);

// Create a smart wallet
const createSmartWalletTxn = await lazorkitProgram.createSmartWalletTxn(
  passkeyPubkey,
  initRuleIns,
  payer.publicKey
);
```

## Features

### Smart Wallet Management
- Create and manage smart wallets
- Secp256r1 authentication
- Configurable wallet rules

### Default Rule System
- Implement default transaction rules
- Custom rule program support
- Whitelist functionality

### Transfer Limits
- Configurable transfer limits
- Token transfer restrictions
- Custom limit rules

## Contributing

1. Fork the repository
2. Create your feature branch
3. Commit your changes
4. Push to the branch
5. Create a new Pull Request

## License

[Add your license information here]