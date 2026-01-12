/**
 * Unit tests for serialization utilities
 */

import { describe, it, expect } from 'vitest';
import {
  serializeCreateSmartWalletArgs,
  serializeSignArgs,
  serializeAddAuthorityArgs,
  serializePluginRefs,
  writeInstructionDiscriminator,
} from '../../src/utils/serialization';
import { LazorkitInstruction } from '../../src/instructions/types';
import { RolePermission } from '../../src/types';

describe('Serialization Utilities', () => {
  describe('writeInstructionDiscriminator', () => {
    it('should write discriminator correctly', () => {
      const buffer = new Uint8Array(2);
      writeInstructionDiscriminator(buffer, LazorkitInstruction.CreateSmartWallet);

      expect(buffer[0]).toBe(0); // Little-endian u16
      expect(buffer[1]).toBe(0);
    });

    it('should write different discriminators', () => {
      const buffer = new Uint8Array(2);
      writeInstructionDiscriminator(buffer, LazorkitInstruction.Sign);

      expect(buffer[0]).toBe(1);
      expect(buffer[1]).toBe(0);
    });
  });

  describe('serializeCreateSmartWalletArgs', () => {
    it('should serialize CreateSmartWallet args correctly', () => {
      const walletId = new Uint8Array(32);
      crypto.getRandomValues(walletId);

      const args = {
        id: walletId,
        bump: 255,
        walletBump: 128,
        firstAuthorityType: 1, // Ed25519
        firstAuthorityDataLen: 32,
        numPluginRefs: 0,
        rolePermission: RolePermission.AllButManageAuthority,
      };

      const serialized = serializeCreateSmartWalletArgs(args);

      expect(serialized.length).toBe(43);
      expect(serialized[32]).toBe(255); // bump
      expect(serialized[33]).toBe(128); // wallet_bump
    });

    it('should throw error for invalid wallet ID', () => {
      const invalidId = new Uint8Array(31);

      expect(() => {
        serializeCreateSmartWalletArgs({
          id: invalidId,
          bump: 0,
          walletBump: 0,
          firstAuthorityType: 1,
          firstAuthorityDataLen: 32,
          numPluginRefs: 0,
          rolePermission: RolePermission.AllButManageAuthority,
        });
      }).toThrow();
    });
  });

  describe('serializeSignArgs', () => {
    it('should serialize Sign args correctly', () => {
      const args = {
        instructionPayloadLen: 100,
        authorityId: 0,
      };

      const serialized = serializeSignArgs(args);

      expect(serialized.length).toBe(6);
      expect(serialized[0]).toBe(100); // payload_len (little-endian)
      expect(serialized[1]).toBe(0);
      expect(serialized[2]).toBe(0); // authority_id (little-endian)
      expect(serialized[3]).toBe(0);
      expect(serialized[4]).toBe(0);
      expect(serialized[5]).toBe(0);
    });
  });

  describe('serializeAddAuthorityArgs', () => {
    it('should serialize AddAuthority args correctly', () => {
      const args = {
        actingAuthorityId: 0,
        newAuthorityType: 1,
        newAuthorityDataLen: 32,
        numPluginRefs: 0,
        rolePermission: RolePermission.ExecuteOnly,
      };

      const serialized = serializeAddAuthorityArgs(args);

      expect(serialized.length).toBe(14);
      expect(serialized[4]).toBe(1); // new_authority_type (little-endian)
      expect(serialized[5]).toBe(0);
    });
  });

  describe('serializePluginRefs', () => {
    it('should serialize plugin refs correctly', () => {
      const pluginRefs = [
        { pluginIndex: 0, priority: 0, enabled: true },
        { pluginIndex: 1, priority: 1, enabled: false },
      ];

      const serialized = serializePluginRefs(pluginRefs);

      expect(serialized.length).toBe(16); // 2 refs * 8 bytes each
      expect(serialized[0]).toBe(0); // First ref: plugin_index (little-endian)
      expect(serialized[1]).toBe(0);
      expect(serialized[2]).toBe(0); // priority
      expect(serialized[3]).toBe(1); // enabled
    });

    it('should handle empty plugin refs', () => {
      const serialized = serializePluginRefs([]);
      expect(serialized.length).toBe(0);
    });
  });
});
