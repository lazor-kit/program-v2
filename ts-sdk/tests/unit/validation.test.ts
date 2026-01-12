/**
 * Unit tests for validation utilities
 */

import { describe, it, expect } from 'vitest';
import {
  isValidAuthorityType,
  assertIsAuthorityType,
  isValidRolePermission,
  assertIsRolePermission,
  isValidWalletId,
  assertIsWalletId,
  isValidDiscriminator,
  assertIsDiscriminator,
} from '../../src/types/validation';
import { AuthorityType, RolePermission, Discriminator } from '../../src/types';
import { LazorkitError } from '../../src/errors';

describe('Validation Utilities', () => {
  describe('AuthorityType validation', () => {
    it('should validate valid authority types', () => {
      expect(isValidAuthorityType(AuthorityType.Ed25519)).toBe(true);
      expect(isValidAuthorityType(AuthorityType.Secp256k1)).toBe(true);
      expect(isValidAuthorityType(AuthorityType.Secp256r1)).toBe(true);
    });

    it('should reject invalid authority types', () => {
      expect(isValidAuthorityType(999)).toBe(false);
      expect(isValidAuthorityType(-1)).toBe(false);
    });

    it('should assert valid authority type', () => {
      expect(() => assertIsAuthorityType(AuthorityType.Ed25519)).not.toThrow();
    });

    it('should throw on invalid authority type', () => {
      expect(() => assertIsAuthorityType(999)).toThrow(LazorkitError);
    });
  });

  describe('RolePermission validation', () => {
    it('should validate valid role permissions', () => {
      expect(isValidRolePermission(RolePermission.All)).toBe(true);
      expect(isValidRolePermission(RolePermission.ExecuteOnly)).toBe(true);
    });

    it('should reject invalid role permissions', () => {
      expect(isValidRolePermission(999)).toBe(false);
    });

    it('should assert valid role permission', () => {
      expect(() => assertIsRolePermission(RolePermission.All)).not.toThrow();
    });

    it('should throw on invalid role permission', () => {
      expect(() => assertIsRolePermission(999)).toThrow(LazorkitError);
    });
  });

  describe('WalletId validation', () => {
    it('should validate 32-byte wallet ID', () => {
      const walletId = new Uint8Array(32);
      expect(isValidWalletId(walletId)).toBe(true);
    });

    it('should reject invalid wallet ID sizes', () => {
      expect(isValidWalletId(new Uint8Array(31))).toBe(false);
      expect(isValidWalletId(new Uint8Array(33))).toBe(false);
    });

    it('should assert valid wallet ID', () => {
      const walletId = new Uint8Array(32);
      expect(() => assertIsWalletId(walletId)).not.toThrow();
    });

    it('should throw on invalid wallet ID', () => {
      const invalidId = new Uint8Array(31);
      expect(() => assertIsWalletId(invalidId)).toThrow(LazorkitError);
    });
  });

  describe('Discriminator validation', () => {
    it('should validate valid discriminators', () => {
      expect(isValidDiscriminator(Discriminator.WalletAccount)).toBe(true);
      expect(isValidDiscriminator(Discriminator.Uninitialized)).toBe(true);
    });

    it('should reject invalid discriminators', () => {
      expect(isValidDiscriminator(999)).toBe(false);
    });

    it('should assert valid discriminator', () => {
      expect(() => assertIsDiscriminator(Discriminator.WalletAccount)).not.toThrow();
    });

    it('should throw on invalid discriminator', () => {
      expect(() => assertIsDiscriminator(999)).toThrow(LazorkitError);
    });
  });
});
