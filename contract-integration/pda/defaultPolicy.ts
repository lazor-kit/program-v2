import { PublicKey } from '@solana/web3.js';
import { Buffer } from 'buffer';

export const POLICY_SEED = Buffer.from('policy');

export function derivePolicyPda(
  programId: PublicKey,
  smartWallet: PublicKey
): PublicKey {
  return PublicKey.findProgramAddressSync(
    [POLICY_SEED, smartWallet.toBuffer()],
    programId
  )[0];
}
