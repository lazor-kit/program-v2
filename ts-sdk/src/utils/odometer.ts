import type { Address } from '@solana/kit';
import type { Rpc } from '@solana/rpc';
import type { GetAccountInfoApi, GetSlotApi } from '@solana/rpc-api';
import { LazorkitError, LazorkitErrorCode } from '../errors';
import { WALLET_ACCOUNT_HEADER_SIZE, NUM_AUTHORITIES_SIZE } from '../types';

/**
 * Fetch odometer from on-chain wallet account
 * 
 * @param rpc - RPC client
 * @param walletAccount - Wallet account address
 * @param authorityId - Authority ID to fetch odometer for
 * @returns Current odometer value
 */
export async function fetchOdometer(
  rpc: Rpc<GetAccountInfoApi & GetSlotApi>,
  walletAccount: Address,
  authorityId: number
): Promise<number> {
  try {
    // Fetch wallet account data
    const { value: accountData } = await rpc.getAccountInfo(walletAccount, {
      encoding: 'base64',
    }).send();

    if (!accountData || !accountData.data) {
      throw new LazorkitError(
        LazorkitErrorCode.InvalidAccountData,
        'Wallet account not found'
      );
    }

    // Parse account data
    const data = typeof accountData.data === 'string'
      ? Buffer.from(accountData.data, 'base64')
      : accountData.data;

    // Skip header: discriminator[1] + bump[1] + id[32] + wallet_bump[1] + version[1] + reserved[4] = 40 bytes
    // Skip num_authorities[2] = 2 bytes
    let offset = WALLET_ACCOUNT_HEADER_SIZE + NUM_AUTHORITIES_SIZE;

    // Read number of authorities (little-endian u16)
    const numAuthorities = Number(data[WALLET_ACCOUNT_HEADER_SIZE]!) | 
                           (Number(data[WALLET_ACCOUNT_HEADER_SIZE + 1]!) << 8);
    
    if (authorityId >= numAuthorities) {
      throw new LazorkitError(
        LazorkitErrorCode.AuthorityNotFound,
        `Authority ID ${authorityId} not found (total authorities: ${numAuthorities})`
      );
    }

    // Skip to the target authority
    // Each authority has a Position struct (16 bytes) followed by authority data
    for (let i = 0; i < authorityId; i++) {
      // Read Position boundary to find next authority
      if (offset + 16 > data.length) {
        throw new LazorkitError(
          LazorkitErrorCode.InvalidAccountData,
          'Invalid account data: cannot read authority position'
        );
      }
      
      // Position struct: authority_type[2] + authority_length[2] + num_plugin_refs[2] + 
      //                  role_permission[1] + id[4] + boundary[4] + padding[1] = 16 bytes
      // Read boundary (little-endian u32) at offset 12
      const boundary = Number(data[offset + 12]!) | 
                       (Number(data[offset + 13]!) << 8) |
                       (Number(data[offset + 14]!) << 16) |
                       (Number(data[offset + 15]!) << 24);
      offset = boundary;
    }

    // Read Position for target authority
    if (offset + 16 > data.length) {
      throw new LazorkitError(
        LazorkitErrorCode.InvalidAccountData,
        'Invalid account data: cannot read target authority position'
      );
    }

    // Read authority type and length (little-endian u16)
    const authorityType = Number(data[offset]!) | (Number(data[offset + 1]!) << 8);
    const authorityLength = Number(data[offset + 2]!) | (Number(data[offset + 3]!) << 8);
    offset += 16; // Skip Position struct

    // Read authority data
    if (offset + authorityLength > data.length) {
      throw new LazorkitError(
        LazorkitErrorCode.InvalidAccountData,
        'Invalid account data: authority data out of bounds'
      );
    }

    // Check if this is Secp256k1 or Secp256r1 (they have odometer)
    // Secp256k1: public_key[33] + padding[3] + signature_odometer[4] = 40 bytes
    // Secp256r1: public_key[33] + padding[3] + signature_odometer[4] = 40 bytes
    if (authorityType === 3 || authorityType === 5) { // Secp256k1 or Secp256r1
      if (authorityLength >= 40) {
        // Odometer is at offset 36 (after public_key[33] + padding[3])
        // Read odometer (little-endian u32)
        const odometer = Number(data[offset + 36]!) |
                        (Number(data[offset + 37]!) << 8) |
                        (Number(data[offset + 38]!) << 16) |
                        (Number(data[offset + 39]!) << 24);
        return odometer;
      }
    }

    // No odometer for this authority type
    return 0;
  } catch (error) {
    if (error instanceof LazorkitError) {
      throw error;
    }
    throw LazorkitError.fromRpcError(error);
  }
}
