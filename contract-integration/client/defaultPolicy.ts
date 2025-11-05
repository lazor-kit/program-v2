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
}
