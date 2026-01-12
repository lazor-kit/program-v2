# Lazorkit V2 SDK Examples

This directory contains usage examples for the Lazorkit V2 TypeScript SDK.

## High-Level API Examples

### `high-level/create-wallet.ts`
Demonstrates how to create a new Lazorkit wallet using the high-level API.

```bash
npx tsx examples/high-level/create-wallet.ts
```

### `high-level/sign-transaction.ts`
Shows how to sign and execute transactions using the high-level API.

```bash
npx tsx examples/high-level/sign-transaction.ts
```

### `high-level/manage-authorities.ts`
Examples for adding, updating, and removing wallet authorities.

```bash
npx tsx examples/high-level/manage-authorities.ts
```

### `high-level/session-management.ts`
Demonstrates session creation and management.

```bash
npx tsx examples/high-level/session-management.ts
```

### `high-level/plugin-management.ts`
Examples for managing wallet plugins.

```bash
npx tsx examples/high-level/plugin-management.ts
```

## Low-Level API Examples

### `low-level/instruction-builder.ts`
Shows how to use the low-level `LazorkitInstructionBuilder` for full control.

```bash
npx tsx examples/low-level/instruction-builder.ts
```

### `low-level/pda-derivation.ts`
Demonstrates PDA derivation utilities.

```bash
npx tsx examples/low-level/pda-derivation.ts
```

## Running Examples

All examples use `tsx` for execution. Make sure you have:

1. Installed dependencies: `npm install`
2. Built the SDK: `npm run build`
3. Set up your RPC endpoint and fee payer address

## Notes

- Examples use placeholder addresses - replace with actual addresses
- Some examples require a running Solana validator or RPC endpoint
- Examples are for demonstration purposes and may need adjustment for production use
