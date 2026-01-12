# Lazorkit V2 SDK Tests

This directory contains unit and integration tests for the Lazorkit V2 TypeScript SDK.

## Test Structure

- `unit/` - Unit tests for individual utilities and functions
- `integration/` - Integration tests that require a running Solana validator

## Running Tests

### All Tests
```bash
npm test
```

### Unit Tests Only
```bash
npm run test:unit
```

### Integration Tests Only
```bash
npm run test:integration
```

### Watch Mode
```bash
npm run test:watch
```

## Integration Tests

Integration tests require:
1. A running Solana validator (local or testnet)
2. Environment variables:
   - `ENABLE_INTEGRATION_TESTS=true`
   - `SOLANA_RPC_URL` (optional, defaults to `http://localhost:8899`)
   - `FEE_PAYER` (optional, for test transactions)

## Test Files

### Unit Tests
- `pda.test.ts` - PDA derivation utilities
- `serialization.test.ts` - Instruction serialization
- `validation.test.ts` - Type validation helpers
- `instructions.test.ts` - Instruction serialization
- `session.test.ts` - Session management utilities

### Integration Tests
- `wallet.test.ts` - LazorkitWallet class integration
- `odometer.test.ts` - Odometer fetching from chain

## Writing Tests

Tests use Vitest. Example:

```typescript
import { describe, it, expect } from 'vitest';
import { findWalletAccount } from '../../src/utils/pda';

describe('PDA Utilities', () => {
  it('should derive wallet account', async () => {
    const walletId = new Uint8Array(32);
    const [address, bump] = await findWalletAccount(walletId);
    expect(address).toBeDefined();
  });
});
```
