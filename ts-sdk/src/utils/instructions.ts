import type { Address } from '@solana/kit';
import { getAddressEncoder } from '@solana/kit';

/**
 * Serialize instructions to compact format
 * 
 * Format: num_instructions[2] + instructions...
 * Each instruction: program_id[32] + num_accounts[1] + accounts... + data_len[2] + data...
 * Each account: address[32]
 */
export async function serializeInstructions(
  instructions: Array<{
    programAddress: Address;
    accounts?: Array<{ address: Address; role?: any }>;
    data?: Uint8Array;
  }>
): Promise<Uint8Array> {
  const addressEncoder = getAddressEncoder();
  const buffers: Uint8Array[] = [];

  // Write number of instructions (u16)
  const numInstructionsBuffer = new Uint8Array(2);
  numInstructionsBuffer[0] = instructions.length & 0xff;
  numInstructionsBuffer[1] = (instructions.length >> 8) & 0xff;
  buffers.push(numInstructionsBuffer);

  for (const instruction of instructions) {
    // Encode program address (32 bytes)
    const programAddressBytes = addressEncoder.encode(instruction.programAddress);
    // addressEncoder returns ReadonlyUint8Array, convert to Uint8Array
    const programBytes = programAddressBytes instanceof Uint8Array 
      ? programAddressBytes 
      : new Uint8Array(programAddressBytes);
    buffers.push(programBytes);

    // Write number of accounts (u8)
    const accounts = instruction.accounts || [];
    const numAccountsBuffer = new Uint8Array(1);
    numAccountsBuffer[0] = accounts.length;
    buffers.push(numAccountsBuffer);

    // Write account addresses (32 bytes each)
    for (const account of accounts) {
      const accountAddressBytes = addressEncoder.encode(account.address);
      // addressEncoder returns ReadonlyUint8Array, convert to Uint8Array
      const accountBytes = accountAddressBytes instanceof Uint8Array 
        ? accountAddressBytes 
        : new Uint8Array(accountAddressBytes);
      buffers.push(accountBytes);
    }

    // Write data length (u16)
    const data = instruction.data || new Uint8Array(0);
    const dataLenBuffer = new Uint8Array(2);
    dataLenBuffer[0] = data.length & 0xff;
    dataLenBuffer[1] = (data.length >> 8) & 0xff;
    buffers.push(dataLenBuffer);

    // Write data
    if (data.length > 0) {
      // Convert ReadonlyUint8Array to Uint8Array if needed
      const dataArray = data instanceof Uint8Array ? data : new Uint8Array(data);
      buffers.push(dataArray);
    }
  }

  // Concatenate all buffers
  const totalLength = buffers.reduce((sum, buf) => sum + buf.length, 0);
  const result = new Uint8Array(totalLength);
  let offset = 0;
  for (const buffer of buffers) {
    result.set(buffer, offset);
    offset += buffer.length;
  }

  return result;
}
