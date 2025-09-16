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

  policyPda(walletDevice: PublicKey): PublicKey {
    return derivePolicyPda(this.programId, walletDevice);
  }

  async buildInitPolicyIx(
    walletId: anchor.BN,
    passkeyPublicKey: number[],
    smartWallet: PublicKey,
    walletDevice: PublicKey
  ): Promise<TransactionInstruction> {
    return await this.program.methods
      .initPolicy(walletId, passkeyPublicKey)
      .accountsPartial({
        smartWallet,
        walletDevice,
        policy: this.policyPda(walletDevice),
        systemProgram: SystemProgram.programId,
      })
      .instruction();
  }

  async buildCheckPolicyIx(
    walletId: anchor.BN,
    passkeyPublicKey: number[],
    walletDevice: PublicKey,
    smartWallet: PublicKey
  ): Promise<TransactionInstruction> {
    return await this.program.methods
      .checkPolicy(walletId, passkeyPublicKey)
      .accountsPartial({
        policy: this.policyPda(walletDevice),
        smartWallet,
        walletDevice,
      })
      .instruction();
  }

  async buildAddDeviceIx(
    smartWallet: PublicKey,
    walletDevice: PublicKey,
    newWalletDevice: PublicKey
  ): Promise<TransactionInstruction> {
    return await this.program.methods
      .addDevice()
      .accountsPartial({
        smartWallet,
        walletDevice,
        newWalletDevice,
        policy: this.policyPda(walletDevice),
        newPolicy: this.policyPda(newWalletDevice),
        systemProgram: SystemProgram.programId,
      })
      .instruction();
  }
}
