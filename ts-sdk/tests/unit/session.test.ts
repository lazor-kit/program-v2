/**
 * Unit tests for session utilities
 */

import { describe, it, expect } from 'vitest';
import {
  generateSessionKey,
  calculateSessionExpiration,
  isSessionExpired,
  getRecommendedSessionDuration,
} from '../../src/utils/session';
import { AuthorityType } from '../../src/types';

describe('Session Utilities', () => {
  describe('generateSessionKey', () => {
    it('should generate 32-byte session key', () => {
      const key = generateSessionKey();

      expect(key.length).toBe(32);
      expect(key).toBeInstanceOf(Uint8Array);
    });

    it('should generate different keys each time', () => {
      const key1 = generateSessionKey();
      const key2 = generateSessionKey();

      // Very unlikely to be the same (1 in 2^256)
      expect(key1).not.toEqual(key2);
    });
  });

  describe('calculateSessionExpiration', () => {
    it('should calculate expiration correctly', () => {
      const currentSlot = 1000n;
      const duration = 500n;

      const expiration = calculateSessionExpiration(currentSlot, duration);

      expect(expiration).toBe(1500n);
    });

    it('should handle large slot numbers', () => {
      const currentSlot = 1000000n;
      const duration = 10000n;

      const expiration = calculateSessionExpiration(currentSlot, duration);

      expect(expiration).toBe(1010000n);
    });
  });

  describe('isSessionExpired', () => {
    it('should detect expired session', () => {
      const expirationSlot = 1000n;
      const currentSlot = 1500n;

      expect(isSessionExpired(expirationSlot, currentSlot)).toBe(true);
    });

    it('should detect active session', () => {
      const expirationSlot = 2000n;
      const currentSlot = 1500n;

      expect(isSessionExpired(expirationSlot, currentSlot)).toBe(false);
    });

    it('should detect session expiring at current slot', () => {
      const expirationSlot = 1000n;
      const currentSlot = 1000n;

      expect(isSessionExpired(expirationSlot, currentSlot)).toBe(false);
    });
  });

  describe('getRecommendedSessionDuration', () => {
    it('should return recommended duration for Ed25519Session', () => {
      const duration = getRecommendedSessionDuration(AuthorityType.Ed25519Session);
      expect(duration).toBe(1000n);
    });

    it('should return recommended duration for Secp256k1Session', () => {
      const duration = getRecommendedSessionDuration(AuthorityType.Secp256k1Session);
      expect(duration).toBe(1000n);
    });

    it('should return default duration for unknown types', () => {
      const duration = getRecommendedSessionDuration(AuthorityType.None);
      expect(duration).toBe(1000n);
    });
  });
});
