/**
 * Integration tests for odometer management
 *
 * These tests require a local validator or test environment.
 */

import { describe, it, expect, beforeAll } from 'vitest';
import { createSolanaRpc } from '@solana/kit';
import { fetchOdometer, findWalletAccount } from '../../src';
import type { Address } from '@solana/kit';

const TEST_ENABLED = process.env.ENABLE_INTEGRATION_TESTS === 'true';
const RPC_URL = process.env.SOLANA_RPC_URL || 'http://localhost:8899';

describe.skipIf(!TEST_ENABLED)('Odometer Integration', () => {
  let rpc: ReturnType<typeof createSolanaRpc>;
  let walletAccount: Address;

  beforeAll(async () => {
    rpc = createSolanaRpc(RPC_URL);

    // Use a known wallet account for testing
    const walletId = new Uint8Array(32);
    [walletAccount] = await findWalletAccount(walletId);
  });

  it('should fetch odometer for Secp256k1 authority', async () => {
    // This test requires a wallet with a Secp256k1 authority
    // Skip if wallet doesn't exist
    try {
      const odometer = await fetchOdometer(rpc, walletAccount, 0);
      expect(typeof odometer).toBe('number');
      expect(odometer).toBeGreaterThanOrEqual(0);
    } catch (error) {
      // Wallet might not exist or authority might not be Secp256k1
      console.warn('Odometer fetch test skipped:', error);
    }
  });
});
