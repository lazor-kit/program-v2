import * as anchor from '@coral-xyz/anchor';
import DefaultPolicyIdl from '../anchor/idl/default_policy.json';
import { DefaultPolicy } from '../anchor/types/default_policy';
import { derivePolicyPda } from '../pda/defaultPolicy';

export class DefaultPolicyClient {
  readonly connection: anchor.web3.Connection;
  readonly program: anchor.Program<DefaultPolicy>;
  readonly programId: anchor.web3.PublicKey;

  constructor(connection: anchor.web3.Connection) {
    this.connection = connection;

    this.program = new anchor.Program<DefaultPolicy>(
      DefaultPolicyIdl as DefaultPolicy,
      {
        connection: connection,
      }
    );
    this.programId = this.program.programId;
  }

  policyPda(smartWallet: anchor.web3.PublicKey): anchor.web3.PublicKey {
    return derivePolicyPda(this.programId, smartWallet);
  }

  getPolicyDataSize(): number {
    return 1 + 32 + 4 + 33 + 32;
  }

  async buildInitPolicyIx(
    walletId: anchor.BN,
    passkeyPublicKey: number[],
    credentialHash: number[],
    policySigner: anchor.web3.PublicKey,
    smartWallet: anchor.web3.PublicKey,
    walletState: anchor.web3.PublicKey
  ): Promise<anchor.web3.TransactionInstruction> {
    return await this.program.methods
      .initPolicy(walletId, passkeyPublicKey, credentialHash)
      .accountsPartial({
        smartWallet,
        walletState,
        policySigner,
      })
      .instruction();
  }

  async buildCheckPolicyIx(
    walletId: anchor.BN,
    passkeyPublicKey: number[],
    policySigner: anchor.web3.PublicKey,
    smartWallet: anchor.web3.PublicKey,
    credentialHash: number[],
    policyData: Buffer<ArrayBufferLike>
  ): Promise<anchor.web3.TransactionInstruction> {
    return await this.program.methods
      .checkPolicy(walletId, passkeyPublicKey, credentialHash, policyData)
      .accountsPartial({
        smartWallet,
        policySigner,
      })
      .instruction();
  }

  async buildAddDeviceIx(
    walletId: anchor.BN,
    passkeyPublicKey: number[],
    credentialHash: number[],
    policyData: Buffer<ArrayBufferLike>,
    newPasskeyPublicKey: number[],
    newCredentialHash: number[],
    smartWallet: anchor.web3.PublicKey,
    policySigner: anchor.web3.PublicKey
  ): Promise<anchor.web3.TransactionInstruction> {
    return await this.program.methods
      .addDevice(
        walletId,
        passkeyPublicKey,
        credentialHash,
        policyData,
        newPasskeyPublicKey,
        newCredentialHash
      )
      .accountsPartial({
        smartWallet,
        policySigner,
      })
      .instruction();
  }

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
