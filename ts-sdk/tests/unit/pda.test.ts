/**
 * Unit tests for PDA utilities
 */

import { describe, it, expect } from 'vitest';
import {
  findWalletAccount,
  findWalletVault,
  createWalletAccountSignerSeeds,
  createWalletVaultSignerSeeds,
  LAZORKIT_PROGRAM_ID,
} from '../../src/utils/pda';
import { assertIsWalletId } from '../../src/types/validation';

describe('PDA Utilities', () => {
  describe('findWalletAccount', () => {
    it('should derive wallet account PDA correctly', async () => {
      const walletId = new Uint8Array(32);
      crypto.getRandomValues(walletId);

      const [address, bump] = await findWalletAccount(walletId);

      expect(address).toBeDefined();
      expect(typeof address).toBe('string');
      expect(bump).toBeGreaterThanOrEqual(0);
      expect(bump).toBeLessThanOrEqual(255);
    });

    it('should throw error for invalid wallet ID', async () => {
      const invalidWalletId = new Uint8Array(31); // Wrong size

      await expect(findWalletAccount(invalidWalletId)).rejects.toThrow();
    });

    it('should use custom program ID when provided', async () => {
      const walletId = new Uint8Array(32);
      crypto.getRandomValues(walletId);
      const customProgramId = '11111111111111111111111111111111' as any;

      const [address, bump] = await findWalletAccount(walletId, customProgramId);

      expect(address).toBeDefined();
      expect(bump).toBeGreaterThanOrEqual(0);
    });
  });

  describe('findWalletVault', () => {
    it('should derive wallet vault PDA correctly', async () => {
      const walletId = new Uint8Array(32);
      crypto.getRandomValues(walletId);
      const [walletAccount] = await findWalletAccount(walletId);

      const [vaultAddress, vaultBump] = await findWalletVault(walletAccount);

      expect(vaultAddress).toBeDefined();
      expect(typeof vaultAddress).toBe('string');
      expect(vaultBump).toBeGreaterThanOrEqual(0);
      expect(vaultBump).toBeLessThanOrEqual(255);
    });
  });

  describe('createWalletAccountSignerSeeds', () => {
    it('should create signer seeds correctly', () => {
      const walletId = new Uint8Array(32);
      crypto.getRandomValues(walletId);
      const bump = 255;

      const seeds = createWalletAccountSignerSeeds(walletId, bump);

      expect(seeds).toHaveLength(3);
      expect(seeds[0]).toBeInstanceOf(Uint8Array);
      expect(seeds[1]).toBeInstanceOf(Uint8Array);
      expect(seeds[2]).toBeInstanceOf(Uint8Array);
      expect(seeds[2][0]).toBe(bump);
    });

    it('should throw error for invalid bump', () => {
      const walletId = new Uint8Array(32);
      crypto.getRandomValues(walletId);

      expect(() => createWalletAccountSignerSeeds(walletId, 256)).toThrow();
      expect(() => createWalletAccountSignerSeeds(walletId, -1)).toThrow();
    });
  });

  describe('createWalletVaultSignerSeeds', () => {
    it('should create vault signer seeds correctly', async () => {
      const walletId = new Uint8Array(32);
      crypto.getRandomValues(walletId);
      const [walletAccount] = await findWalletAccount(walletId);
      const bump = 128;

      const seeds = createWalletVaultSignerSeeds(walletAccount, bump);

      expect(seeds).toHaveLength(3);
      expect(seeds[0]).toBeInstanceOf(Uint8Array);
      expect(seeds[1]).toBeInstanceOf(Uint8Array);
      expect(seeds[2]).toBeInstanceOf(Uint8Array);
      expect(seeds[2][0]).toBe(bump);
    });
  });
});
