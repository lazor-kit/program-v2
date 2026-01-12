/**
 * Unit tests for instruction serialization
 */

import { describe, it, expect } from 'vitest';
import { serializeInstructions } from '../../src/utils/instructions';
import type { Address } from '@solana/kit';

describe('Instruction Serialization', () => {
  it('should serialize single instruction', async () => {
    const instructions = [
      {
        programAddress: '11111111111111111111111111111111' as Address,
        accounts: [
          { address: '11111111111111111111111111111112' as Address, role: 'writable' },
          { address: '11111111111111111111111111111113' as Address, role: 'readonly' },
        ],
        data: new Uint8Array([1, 2, 3, 4]),
      },
    ];

    const serialized = await serializeInstructions(instructions);

    // Format: num_instructions[2] + program_id[32] + num_accounts[1] + accounts[32*2] + data_len[2] + data[4]
    expect(serialized.length).toBeGreaterThan(0);
    expect(serialized[0]).toBe(1); // num_instructions (little-endian)
    expect(serialized[1]).toBe(0);
  });

  it('should serialize multiple instructions', async () => {
    const instructions = [
      {
        programAddress: '11111111111111111111111111111111' as Address,
        accounts: [],
        data: new Uint8Array([1]),
      },
      {
        programAddress: 'SysvarRent111111111111111111111111111111111' as Address,
        accounts: [],
        data: new Uint8Array([2]),
      },
    ];

    const serialized = await serializeInstructions(instructions);

    expect(serialized[0]).toBe(2); // num_instructions
    expect(serialized[1]).toBe(0);
  });

  it('should handle instructions without accounts', async () => {
    const instructions = [
      {
        programAddress: '11111111111111111111111111111111' as Address,
        data: new Uint8Array([1, 2, 3]),
      },
    ];

    const serialized = await serializeInstructions(instructions);

    expect(serialized.length).toBeGreaterThan(0);
    // After num_instructions[2] + program_id[32] + num_accounts[1] = 35 bytes
    // num_accounts should be 0
    expect(serialized[34]).toBe(0);
  });

  it('should handle instructions without data', async () => {
    const instructions = [
      {
        programAddress: '11111111111111111111111111111111' as Address,
        accounts: [],
      },
    ];

    const serialized = await serializeInstructions(instructions);

    expect(serialized.length).toBeGreaterThan(0);
  });
});
