# LazorKit SDK Examples

This directory contains example code demonstrating how to use the LazorKit SDK for common operations.

## Examples

### 1. Create Wallet (`create_wallet.rs`)
Shows how to create a new LazorKit smart wallet with an Ed25519 owner.

```bash
cargo run --example create_wallet
```

### 2. Fetch Wallet (`fetch_wallet.rs`)
Demonstrates fetching wallet state from the blockchain and listing all roles.

```bash
cargo run --example fetch_wallet
```

### 3. Add Authority (`add_authority.rs`)
Shows how to add a new admin or spender role to an existing wallet.

```bash
cargo run --example add_authority
```

### 4. Create Session (`create_session.rs`)
Demonstrates creating a temporary session key for a role.

```bash
cargo run --example create_session
```

## Prerequisites

To run these examples, you'll need to implement a `SolConnection` trait. The examples use placeholder connections.

## Next Steps

1. Implement your `SolConnection` using `solana-client`
2. Uncomment the connection code in examples
3. Replace placeholder addresses with real PDAs
4. Fund accounts with SOL for rent

## Learn More

- See the main [README](../README.md) for SDK documentation
- Check [ARCHITECTURE.md](../../../docs/ARCHITECTURE.md) for contract details
