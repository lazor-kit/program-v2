import * as anchor from '@coral-xyz/anchor';
import {
  deriveSmartWalletPda,
  deriveSmartWalletConfigPda,
  deriveChunkPda,
  deriveWalletAuthorityPda,
} from '../../pda/lazorkit';
import * as types from '../../types';
import {
  assertValidCredentialHash,
  assertValidPublicKey,
  assertPositiveBN,
} from '../../validation';

type PublicKey = anchor.web3.PublicKey;
type BN = anchor.BN;

/**
 * Helper responsible for deriving PDA addresses tied to the LazorKit program.
 * Centralizing these derivations keeps the main client small and ensures
 * consistent validation for every caller.
 */
export class WalletPdaFactory {
  constructor(private readonly programId: PublicKey) { }

  smartWallet(baseSeed: number[], salt: BN): PublicKey {
    return deriveSmartWalletPda(this.programId, baseSeed, salt);
  }

  walletState(smartWallet: PublicKey): PublicKey {
    assertValidPublicKey(smartWallet, 'smartWallet');
    return deriveSmartWalletConfigPda(this.programId, smartWallet);
  }

  walletAuthority(
    smartWallet: PublicKey,
    credentialHash: types.CredentialHash | number[]
  ): PublicKey {
    assertValidPublicKey(smartWallet, 'smartWallet');
    assertValidCredentialHash(credentialHash, 'credentialHash');

    return deriveWalletAuthorityPda(
      this.programId,
      smartWallet,
      credentialHash
    )[0];
  }

  chunk(smartWallet: PublicKey, nonce: BN): PublicKey {
    assertValidPublicKey(smartWallet, 'smartWallet');
    return deriveChunkPda(this.programId, smartWallet, nonce);
  }
}

