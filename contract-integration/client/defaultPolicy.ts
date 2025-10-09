import * as anchor from '@coral-xyz/anchor';
import {
  Connection,
  PublicKey,
  SystemProgram,
  TransactionInstruction,
} from '@solana/web3.js';
import DefaultPolicyIdl from '../anchor/idl/default_policy.json';
import { DefaultPolicy } from '../anchor/types/default_policy';
import { derivePolicyPda } from '../pda/defaultPolicy';

export class DefaultPolicyClient {
  readonly connection: Connection;
  readonly program: anchor.Program<DefaultPolicy>;
  readonly programId: PublicKey;

  constructor(connection: Connection) {
    this.connection = connection;

    this.program = new anchor.Program<DefaultPolicy>(
      DefaultPolicyIdl as DefaultPolicy,
      {
        connection: connection,
      }
    );
    this.programId = this.program.programId;
  }

  policyPda(smartWallet: PublicKey): PublicKey {
    return derivePolicyPda(this.programId, smartWallet);
  }

  async buildInitPolicyIx(
    walletId: anchor.BN,
    passkeyPublicKey: number[],
    credentialHash: number[],
    smartWallet: PublicKey,
    walletState: PublicKey
  ): Promise<TransactionInstruction> {
    return await this.program.methods
      .initPolicy(walletId, passkeyPublicKey, credentialHash)
      .accountsPartial({
        smartWallet,
        walletState,
      })
      .instruction();
  }

  async buildCheckPolicyIx(
    walletId: anchor.BN,
    passkeyPublicKey: number[],
    walletDevice: PublicKey,
    smartWallet: PublicKey,
    credentialHash: number[],
    policyData: Buffer<ArrayBufferLike>
  ): Promise<TransactionInstruction> {
    return await this.program.methods
      .checkPolicy(walletId, passkeyPublicKey, credentialHash, policyData)
      .accountsPartial({
        smartWallet,
        walletDevice,
      })
      .instruction();
  }

  // async buildAddDeviceIx(
  //   walletId: anchor.BN,
  //   passkeyPublicKey: number[],
  //   newPasskeyPublicKey: number[],
  //   smartWallet: PublicKey,
  //   walletDevice: PublicKey,
  //   newWalletDevice: PublicKey
  // ): Promise<TransactionInstruction> {
  //   return await this.program.methods
  //     .addDevice(walletId, passkeyPublicKey, newPasskeyPublicKey)
  //     .accountsPartial({
  //       smartWallet,
  //       walletDevice,
  //       newWalletDevice,
  //       policy: this.policyPda(smartWallet),
  //     })
  //     .instruction();
  // }

  // async buildRemoveDeviceIx(
  //   walletId: anchor.BN,
  //   passkeyPublicKey: number[],
  //   removePasskeyPublicKey: number[],
  //   smartWallet: PublicKey,
  //   walletDevice: PublicKey,
  //   rmWalletDevice: PublicKey
  // ): Promise<TransactionInstruction> {
  //   return await this.program.methods
  //     .removeDevice(walletId, passkeyPublicKey, removePasskeyPublicKey)
  //     .accountsPartial({
  //       smartWallet,
  //       walletDevice,
  //       rmWalletDevice,
  //       policy: this.policyPda(smartWallet),
  //     })
  //     .instruction();
  // }

  // async buildDestroyPolicyIx(
  //   walletId: anchor.BN,
  //   passkeyPublicKey: number[],
  //   smartWallet: PublicKey,
  //   walletDevice: PublicKey
  // ): Promise<TransactionInstruction> {
  //   return await this.program.methods
  //     .destroyPolicy(walletId, passkeyPublicKey)
  //     .accountsPartial({
  //       smartWallet,
  //       walletDevice,
  //       policy: this.policyPda(smartWallet),
  //     })
  //     .instruction();
  // }
}
