/**
 * LazorWeb3Client — High-level wrapper for LazorKit instructions using @solana/web3.js v1.
 *
 * IMPLEMENTATION NOTE:
 * We manually encode instruction data for instructions with `bytes` fields (CreateWallet,
 * AddAuthority, TransferOwnership, Execute) because the Solita-generated serializers
 * use `beet.bytes` which adds a 4-byte length prefix, but the LazorKit contract
 * expects raw fixed-size byte arrays (C-struct style).
 */

import {
  PublicKey,
  TransactionInstruction,
  SystemProgram,
  SYSVAR_RENT_PUBKEY,
  type AccountMeta,
} from "@solana/web3.js";

import {
  createCloseWalletInstruction,
  createCreateSessionInstruction,
  createCloseSessionInstruction,
  createRemoveAuthorityInstruction,
  createInitializeConfigInstruction,
  createInitTreasuryShardInstruction,
  createSweepTreasuryInstruction,
  PROGRAM_ID,
} from "../generated";

import { packCompactInstructions, type CompactInstruction } from "./packing";
import { findAuthorityPda } from "./pdas";

export class LazorWeb3Client {
  constructor(private programId: PublicKey = PROGRAM_ID) { }

  private getAuthPayload(
    authType: number,
    authPubkey: Uint8Array,
    credentialHash: Uint8Array
  ): Uint8Array {
    if (authType === 1) { // Secp256r1
      // 32 bytes hash + 33 bytes key
      const payload = new Uint8Array(65);
      payload.set(credentialHash.slice(0, 32), 0);
      payload.set(authPubkey.slice(0, 33), 32);
      return payload;
    } else { // Ed25519
      // 32 bytes key
      return new Uint8Array(authPubkey.slice(0, 32));
    }
  }

  // ─── Wallet ──────────────────────────────────────────────────────

  /**
   * CreateWallet — manually serializes instruction data to avoid Solita's bytes prefix.
   *
   * On-chain layout:
   * [disc: u8(0)] [userSeed: 32] [authType: u8] [authBump: u8] [padding: 6] [payload: bytes]
   */
  createWallet(params: {
    payer: PublicKey;
    wallet: PublicKey;
    vault: PublicKey;
    authority: PublicKey;
    config: PublicKey;
    treasuryShard: PublicKey;
    userSeed: Uint8Array;
    authType: number;
    authBump?: number;
    authPubkey: Uint8Array;
    credentialHash?: Uint8Array;
  }): TransactionInstruction {
    const authBump = params.authBump || 0;
    const padding = new Uint8Array(6).fill(0);
    const payload = this.getAuthPayload(
      params.authType,
      params.authPubkey,
      params.credentialHash || new Uint8Array(32)
    );

    const data = Buffer.alloc(1 + 32 + 1 + 1 + 6 + payload.length);
    let offset = 0;

    data.writeUInt8(0, offset); offset += 1; // disc
    data.set(params.userSeed.slice(0, 32), offset); offset += 32;
    data.writeUInt8(params.authType, offset); offset += 1;
    data.writeUInt8(authBump, offset); offset += 1;
    data.set(padding, offset); offset += 6;
    data.set(payload, offset);

    const keys: AccountMeta[] = [
      { pubkey: params.payer, isWritable: true, isSigner: true },
      { pubkey: params.wallet, isWritable: true, isSigner: false },
      { pubkey: params.vault, isWritable: true, isSigner: false },
      { pubkey: params.authority, isWritable: true, isSigner: false },
      { pubkey: SystemProgram.programId, isWritable: false, isSigner: false },
      { pubkey: SYSVAR_RENT_PUBKEY, isWritable: false, isSigner: false },
      { pubkey: params.config, isWritable: false, isSigner: false },
      { pubkey: params.treasuryShard, isWritable: true, isSigner: false },
    ];

    return new TransactionInstruction({
      programId: this.programId,
      keys,
      data,
    });
  }

  closeWallet(params: {
    payer: PublicKey;
    wallet: PublicKey;
    vault: PublicKey;
    ownerAuthority: PublicKey;
    destination: PublicKey;
    ownerSigner?: PublicKey;
    sysvarInstructions?: PublicKey;
  }): TransactionInstruction {
    return createCloseWalletInstruction(
      {
        payer: params.payer,
        wallet: params.wallet,
        vault: params.vault,
        ownerAuthority: params.ownerAuthority,
        destination: params.destination,
        ownerSigner: params.ownerSigner,
        sysvarInstructions: params.sysvarInstructions,
      },
      this.programId
    );
  }

  // ─── Authority ───────────────────────────────────────────────────

  /**
   * AddAuthority — manually serializes to avoid prefix.
   * Layout: [disc(1)][type(1)][role(1)][pad(6)][payload(Ed=32, Secp=65)]
   */
  addAuthority(params: {
    payer: PublicKey;
    wallet: PublicKey;
    adminAuthority: PublicKey;
    newAuthority: PublicKey;
    config: PublicKey;
    treasuryShard: PublicKey;
    authType: number;
    newRole: number;
    authPubkey: Uint8Array;
    credentialHash?: Uint8Array;
    authorizerSigner?: PublicKey;
  }): TransactionInstruction {
    const padding = new Uint8Array(6).fill(0);
    const payload = this.getAuthPayload(
      params.authType,
      params.authPubkey,
      params.credentialHash || new Uint8Array(32)
    );

    const data = Buffer.alloc(1 + 1 + 1 + 6 + payload.length);
    let offset = 0;

    data.writeUInt8(1, offset); offset += 1; // disc
    data.writeUInt8(params.authType, offset); offset += 1;
    data.writeUInt8(params.newRole, offset); offset += 1;
    data.set(padding, offset); offset += 6;
    data.set(payload, offset);

    const keys: AccountMeta[] = [
      { pubkey: params.payer, isWritable: true, isSigner: true },
      { pubkey: params.wallet, isWritable: false, isSigner: false },
      { pubkey: params.adminAuthority, isWritable: true, isSigner: false },
      { pubkey: params.newAuthority, isWritable: true, isSigner: false },
      { pubkey: SystemProgram.programId, isWritable: false, isSigner: false },
      { pubkey: params.config, isWritable: false, isSigner: false },
      { pubkey: params.treasuryShard, isWritable: true, isSigner: false },
    ];

    if (params.authorizerSigner) {
      keys.push({
        pubkey: params.authorizerSigner,
        isWritable: false,
        isSigner: true,
      });
    }

    return new TransactionInstruction({
      programId: this.programId,
      keys,
      data,
    });
  }

  removeAuthority(params: {
    payer: PublicKey;
    wallet: PublicKey;
    adminAuthority: PublicKey;
    targetAuthority: PublicKey;
    refundDestination: PublicKey;
    config: PublicKey;
    treasuryShard: PublicKey;
    authorizerSigner?: PublicKey;
  }): TransactionInstruction {
    return createRemoveAuthorityInstruction(
      {
        payer: params.payer,
        wallet: params.wallet,
        adminAuthority: params.adminAuthority,
        targetAuthority: params.targetAuthority,
        refundDestination: params.refundDestination,
        config: params.config,
        treasuryShard: params.treasuryShard,
        systemProgram: SystemProgram.programId,
        authorizerSigner: params.authorizerSigner,
      },
      this.programId
    );
  }

  /**
   * TransferOwnership — manually serializes to avoid prefix.
   * Layout: [disc(3)][type(1)][payload(Ed=32, Secp=65)]
   */
  transferOwnership(params: {
    payer: PublicKey;
    wallet: PublicKey;
    currentOwnerAuthority: PublicKey;
    newOwnerAuthority: PublicKey;
    config: PublicKey;
    treasuryShard: PublicKey;
    authType: number;
    authPubkey: Uint8Array;
    credentialHash?: Uint8Array;
    authorizerSigner?: PublicKey;
  }): TransactionInstruction {
    const payload = this.getAuthPayload(
      params.authType,
      params.authPubkey,
      params.credentialHash || new Uint8Array(32)
    );

    const data = Buffer.alloc(1 + 1 + payload.length);
    let offset = 0;

    data.writeUInt8(3, offset); offset += 1; // disc
    data.writeUInt8(params.authType, offset); offset += 1;
    data.set(payload, offset);

    const keys: AccountMeta[] = [
      { pubkey: params.payer, isWritable: true, isSigner: true },
      { pubkey: params.wallet, isWritable: false, isSigner: false },
      { pubkey: params.currentOwnerAuthority, isWritable: true, isSigner: false },
      { pubkey: params.newOwnerAuthority, isWritable: true, isSigner: false },
      { pubkey: SystemProgram.programId, isWritable: false, isSigner: false },
      { pubkey: SYSVAR_RENT_PUBKEY, isWritable: false, isSigner: false },
      { pubkey: params.config, isWritable: false, isSigner: false },
      { pubkey: params.treasuryShard, isWritable: true, isSigner: false },
    ];

    if (params.authorizerSigner) {
      keys.push({
        pubkey: params.authorizerSigner,
        isWritable: false,
        isSigner: true,
      });
    }

    return new TransactionInstruction({
      programId: this.programId,
      keys,
      data,
    });
  }

  // ─── Session ─────────────────────────────────────────────────────

  createSession(params: {
    payer: PublicKey;
    wallet: PublicKey;
    adminAuthority: PublicKey;
    session: PublicKey;
    config: PublicKey;
    treasuryShard: PublicKey;
    sessionKey: Uint8Array | number[];
    expiresAt: bigint | number;
    authorizerSigner?: PublicKey;
  }): TransactionInstruction {
    const sessionKeyArr = Array.isArray(params.sessionKey)
      ? params.sessionKey
      : Array.from(params.sessionKey);

    return createCreateSessionInstruction(
      {
        payer: params.payer,
        wallet: params.wallet,
        adminAuthority: params.adminAuthority,
        session: params.session,
        config: params.config,
        treasuryShard: params.treasuryShard,
        systemProgram: SystemProgram.programId,
        authorizerSigner: params.authorizerSigner,
      },
      {
        sessionKey: sessionKeyArr as number[],
        expiresAt: BigInt(params.expiresAt),
      },
      this.programId
    );
  }

  closeSession(params: {
    payer: PublicKey;
    wallet: PublicKey;
    session: PublicKey;
    config: PublicKey;
    authorizer?: PublicKey;
    authorizerSigner?: PublicKey;
    sysvarInstructions?: PublicKey;
  }): TransactionInstruction {
    return createCloseSessionInstruction(
      {
        payer: params.payer,
        wallet: params.wallet,
        session: params.session,
        config: params.config,
        authorizer: params.authorizer,
        authorizerSigner: params.authorizerSigner,
        sysvarInstructions: params.sysvarInstructions,
      },
      this.programId
    );
  }

  // ─── Execute ─────────────────────────────────────────────────────

  /**
   * Execute — manually builds instruction data.
   * Layout: [disc: u8(4)] [instructions: PackedBytes]
   */
  execute(params: {
    payer: PublicKey;
    wallet: PublicKey;
    authority: PublicKey;
    vault: PublicKey;
    config: PublicKey;
    treasuryShard: PublicKey;
    packedInstructions: Uint8Array;
    authorizerSigner?: PublicKey;
    sysvarInstructions?: PublicKey;
    remainingAccounts?: AccountMeta[];
  }): TransactionInstruction {
    const data = Buffer.alloc(1 + params.packedInstructions.length);
    data[0] = 4; // disc
    data.set(params.packedInstructions, 1);

    const keys: AccountMeta[] = [
      { pubkey: params.payer, isWritable: true, isSigner: true },
      { pubkey: params.wallet, isWritable: false, isSigner: false },
      { pubkey: params.authority, isWritable: true, isSigner: false },
      { pubkey: params.vault, isWritable: true, isSigner: false },
      { pubkey: params.config, isWritable: false, isSigner: false },
      { pubkey: params.treasuryShard, isWritable: true, isSigner: false },
      { pubkey: SystemProgram.programId, isWritable: false, isSigner: false },
    ];

    if (params.sysvarInstructions) {
      keys.push({ pubkey: params.sysvarInstructions, isWritable: false, isSigner: false });
    }

    if (params.remainingAccounts) {
      keys.push(...params.remainingAccounts);
    }

    if (params.authorizerSigner) {
      keys.push({
        pubkey: params.authorizerSigner,
        isWritable: false,
        isSigner: true,
      });
    }

    return new TransactionInstruction({
      programId: this.programId,
      keys,
      data,
    });
  }

  /**
   * buildExecute — High-level builder that deduplicates and maps accounts.
   */
  buildExecute(params: {
    payer: PublicKey;
    wallet: PublicKey;
    authority: PublicKey;
    vault: PublicKey;
    config: PublicKey;
    treasuryShard: PublicKey;
    innerInstructions: TransactionInstruction[];
    authorizerSigner?: PublicKey;
    signature?: Uint8Array;
    sysvarInstructions?: PublicKey;
  }): TransactionInstruction {
    const baseAccounts: PublicKey[] = [
      params.payer,
      params.wallet,
      params.authority,
      params.vault,
      params.config,
      params.treasuryShard,
      SystemProgram.programId,
    ];

    const accountMap = new Map<string, number>();
    const accountMetas: AccountMeta[] = [];

    baseAccounts.forEach((pk, idx) => {
      accountMap.set(pk.toBase58(), idx);
      accountMetas.push({
        pubkey: pk,
        isWritable: idx === 0 || idx === 2 || idx === 3 || idx === 5,
        isSigner: idx === 0,
      });
    });

    const vaultKey = params.vault.toBase58();
    const walletKey = params.wallet.toBase58();

    const addAccount = (pubkey: PublicKey, isSigner: boolean, isWritable: boolean): number => {
      const key = pubkey.toBase58();
      if (key === vaultKey || key === walletKey) isSigner = false;

      if (!accountMap.has(key)) {
        const idx = accountMetas.length;
        accountMap.set(key, idx);
        accountMetas.push({ pubkey, isWritable, isSigner });
        return idx;
      } else {
        const idx = accountMap.get(key)!;
        const existing = accountMetas[idx];
        if (isWritable) existing.isWritable = true;
        if (isSigner) existing.isSigner = true;
        return idx;
      }
    };

    const compactIxs: CompactInstruction[] = [];
    for (const ix of params.innerInstructions) {
      const programIdIndex = addAccount(ix.programId, false, false);
      const accountIndexes: number[] = [];
      for (const acc of ix.keys) {
        accountIndexes.push(addAccount(acc.pubkey, acc.isSigner, acc.isWritable));
      }
      compactIxs.push({ programIdIndex, accountIndexes, data: ix.data });
    }

    const packed = packCompactInstructions(compactIxs);
    const sig = params.signature;
    const dataSize = 1 + packed.length + (sig ? sig.length : 0);
    const data = Buffer.alloc(dataSize);
    data[0] = 4; // disc
    data.set(packed, 1);
    if (sig) data.set(sig, 1 + packed.length);

    if (params.sysvarInstructions) {
      addAccount(params.sysvarInstructions, false, false);
    }

    if (params.authorizerSigner) {
      addAccount(params.authorizerSigner, true, false);
    }

    return new TransactionInstruction({
      programId: this.programId,
      keys: accountMetas,
      data,
    });
  }

  // ─── Admin ───────────────────────────────────────────────────────

  initializeConfig(params: {
    admin: PublicKey;
    config: PublicKey;
    walletFee: bigint | number;
    actionFee: bigint | number;
    numShards: number;
  }): TransactionInstruction {
    return createInitializeConfigInstruction(
      {
        admin: params.admin,
        config: params.config,
        systemProgram: SystemProgram.programId,
        rent: SYSVAR_RENT_PUBKEY,
      },
      {
        walletFee: BigInt(params.walletFee),
        actionFee: BigInt(params.actionFee),
        numShards: params.numShards,
      },
      this.programId
    );
  }

  initTreasuryShard(params: {
    payer: PublicKey;
    config: PublicKey;
    treasuryShard: PublicKey;
    shardId: number;
  }): TransactionInstruction {
    return createInitTreasuryShardInstruction(
      {
        payer: params.payer,
        config: params.config,
        treasuryShard: params.treasuryShard,
        systemProgram: SystemProgram.programId,
        rent: SYSVAR_RENT_PUBKEY,
      },
      {
        shardId: params.shardId,
      },
      this.programId
    );
  }

  sweepTreasury(params: {
    admin: PublicKey;
    config: PublicKey;
    treasuryShard: PublicKey;
    destination: PublicKey;
    shardId: number;
  }): TransactionInstruction {
    return createSweepTreasuryInstruction(
      {
        admin: params.admin,
        config: params.config,
        treasuryShard: params.treasuryShard,
        destination: params.destination,
      },
      {
        shardId: params.shardId,
      },
      this.programId
    );
  }

  // ─── Utility helpers ─────────────────────────────────────────────

  async getAuthorityByPublicKey(
    connection: any,
    walletAddress: PublicKey,
    pubkey: PublicKey
  ): Promise<any | null> {
    const [pda] = findAuthorityPda(walletAddress, pubkey.toBytes(), this.programId);
    try {
      const accountInfo = await connection.getAccountInfo(pda);
      if (!accountInfo) return null;
      return { address: pda, data: accountInfo.data };
    } catch {
      return null;
    }
  }
}
