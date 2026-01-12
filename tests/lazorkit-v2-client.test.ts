import * as anchor from '@coral-xyz/anchor';
import { expect } from 'chai';
import { Buffer } from 'buffer';
import {
  LazorkitV2Client,
  LAZORKIT_V2_PROGRAM_ID,
} from '../sdk/client/lazorkit-v2';
import {
  AuthorityType,
  PluginType,
  CreateWalletParams,
  AddAuthorityParams,
  AddPluginParams,
  SignParams,
} from '../sdk/types/lazorkit-v2';

describe('LazorkitV2Client', () => {
  let client: LazorkitV2Client;
  let connection: anchor.web3.Connection;
  let payer: anchor.web3.Keypair;
  let walletId: Uint8Array;

  beforeEach(() => {
    // Use localhost or testnet connection
    connection = new anchor.web3.Connection(
      'http://localhost:8899',
      'confirmed'
    );
    client = new LazorkitV2Client(connection);
    payer = anchor.web3.Keypair.generate();
    
    // Generate random wallet ID
    walletId = new Uint8Array(32);
    crypto.getRandomValues(walletId);
  });

  describe('PDA Derivation', () => {
    it('should derive WalletAccount PDA correctly', () => {
      const [walletAccount, bump] = client.deriveWalletAccount(walletId);
      
      expect(walletAccount).to.be.instanceOf(anchor.web3.PublicKey);
      expect(bump).to.be.a('number');
      expect(bump).to.be.at.least(0);
      expect(bump).to.be.at.most(255);
      
      // Verify PDA derivation
      const [expectedPda, expectedBump] = anchor.web3.PublicKey.findProgramAddressSync(
        [Buffer.from('wallet_account'), Buffer.from(walletId)],
        LAZORKIT_V2_PROGRAM_ID
      );
      
      expect(walletAccount.toBase58()).to.equal(expectedPda.toBase58());
      expect(bump).to.equal(expectedBump);
    });

    it('should derive Wallet Vault PDA correctly', () => {
      const [walletAccount] = client.deriveWalletAccount(walletId);
      const [walletVault, vaultBump] = client.deriveWalletVault(walletAccount);
      
      expect(walletVault).to.be.instanceOf(anchor.web3.PublicKey);
      expect(vaultBump).to.be.a('number');
      
      // Verify PDA derivation
      const [expectedVault, expectedBump] = anchor.web3.PublicKey.findProgramAddressSync(
        [Buffer.from('wallet_vault'), walletAccount.toBuffer()],
        anchor.web3.SystemProgram.programId
      );
      
      expect(walletVault.toBase58()).to.equal(expectedVault.toBase58());
      expect(vaultBump).to.equal(expectedBump);
    });

    it('should derive Plugin Config PDA correctly', () => {
      const [walletAccount] = client.deriveWalletAccount(walletId);
      const pluginProgramId = anchor.web3.Keypair.generate().publicKey;
      const pluginSeed = 'role_permission_config';
      
      const [pluginConfig, configBump] = client.derivePluginConfig(
        pluginProgramId,
        pluginSeed,
        walletAccount
      );
      
      expect(pluginConfig).to.be.instanceOf(anchor.web3.PublicKey);
      expect(configBump).to.be.a('number');
      
      // Verify PDA derivation
      const [expectedConfig, expectedBump] = anchor.web3.PublicKey.findProgramAddressSync(
        [Buffer.from(pluginSeed), walletAccount.toBuffer()],
        pluginProgramId
      );
      
      expect(pluginConfig.toBase58()).to.equal(expectedConfig.toBase58());
      expect(configBump).to.equal(expectedBump);
    });
  });

  describe('Authority Serialization', () => {
    it('should serialize Ed25519 authority correctly', () => {
      const publicKey = anchor.web3.Keypair.generate().publicKey;
      const serialized = client.serializeEd25519Authority(publicKey);
      
      expect(serialized).to.be.instanceOf(Buffer);
      expect(serialized.length).to.equal(32);
      expect(serialized).to.deep.equal(Buffer.from(publicKey.toBytes()));
    });

    it('should serialize Secp256k1 authority correctly', () => {
      const publicKey = Buffer.alloc(64);
      crypto.getRandomValues(publicKey);
      
      const serialized = client.serializeSecp256k1Authority(publicKey);
      
      expect(serialized).to.be.instanceOf(Buffer);
      expect(serialized.length).to.equal(64);
      expect(serialized).to.deep.equal(publicKey);
    });

    it('should throw error for invalid Secp256k1 key length', () => {
      const invalidKey = Buffer.alloc(32);
      
      expect(() => {
        client.serializeSecp256k1Authority(invalidKey);
      }).to.throw('Secp256k1 public key must be 64 bytes');
    });

    it('should serialize Secp256r1 authority correctly', () => {
      const publicKey = Buffer.alloc(33);
      crypto.getRandomValues(publicKey);
      
      const serialized = client.serializeSecp256r1Authority(publicKey);
      
      expect(serialized).to.be.instanceOf(Buffer);
      expect(serialized.length).to.equal(33);
      expect(serialized).to.deep.equal(publicKey);
    });

    it('should throw error for invalid Secp256r1 key length', () => {
      const invalidKey = Buffer.alloc(32);
      
      expect(() => {
        client.serializeSecp256r1Authority(invalidKey);
      }).to.throw('Secp256r1 public key must be 33 bytes');
    });

    it('should serialize ProgramExec authority correctly', () => {
      const programId = anchor.web3.Keypair.generate().publicKey;
      const serialized = client.serializeProgramExecAuthority(programId);
      
      expect(serialized).to.be.instanceOf(Buffer);
      expect(serialized.length).to.equal(32);
      expect(serialized).to.deep.equal(Buffer.from(programId.toBytes()));
    });
  });

  describe('Instruction Building', () => {
    it('should build CreateSmartWallet instruction', () => {
      const instruction = client.buildCreateWalletInstruction(
        { id: walletId },
        payer.publicKey
      );
      
      expect(instruction).to.be.instanceOf(anchor.web3.TransactionInstruction);
      expect(instruction.programId.toBase58()).to.equal(LAZORKIT_V2_PROGRAM_ID.toBase58());
      expect(instruction.keys.length).to.equal(4);
      expect(instruction.keys[0].isWritable).to.be.true;
      expect(instruction.keys[1].isWritable).to.be.true;
      expect(instruction.keys[2].isSigner).to.be.true;
      expect(instruction.keys[2].isWritable).to.be.true;
      
      // Verify instruction data structure
      expect(instruction.data.length).to.be.at.least(42);
      const discriminator = instruction.data.readUInt16LE(0);
      expect(discriminator).to.equal(0); // CreateSmartWallet = 0
    });

    it('should build AddAuthority instruction for Ed25519', () => {
      const authorityKeypair = anchor.web3.Keypair.generate();
      const authorityData = client.serializeEd25519Authority(authorityKeypair.publicKey);
      
      const params: AddAuthorityParams = {
        authorityType: AuthorityType.Ed25519,
        authorityData: authorityData,
      };
      
      const instruction = client.buildAddAuthorityInstruction(
        walletId,
        params,
        payer.publicKey
      );
      
      expect(instruction).to.be.instanceOf(anchor.web3.TransactionInstruction);
      expect(instruction.programId.toBase58()).to.equal(LAZORKIT_V2_PROGRAM_ID.toBase58());
      expect(instruction.keys.length).to.equal(3);
      
      // Verify instruction data
      const discriminator = instruction.data.readUInt16LE(0);
      expect(discriminator).to.equal(2); // AddAuthority = 2
      
      const authorityType = instruction.data.readUInt16LE(2);
      expect(authorityType).to.equal(AuthorityType.Ed25519);
    });

    it('should build AddAuthority instruction with plugin refs', () => {
      const authorityKeypair = anchor.web3.Keypair.generate();
      const authorityData = client.serializeEd25519Authority(authorityKeypair.publicKey);
      
      const params: AddAuthorityParams = {
        authorityType: AuthorityType.Ed25519,
        authorityData: authorityData,
        pluginRefs: [
          {
            pluginIndex: 0,
            priority: 0,
            enabled: 1,
          },
          {
            pluginIndex: 1,
            priority: 1,
            enabled: 1,
          },
        ],
      };
      
      const instruction = client.buildAddAuthorityInstruction(
        walletId,
        params,
        payer.publicKey
      );
      
      expect(instruction).to.be.instanceOf(anchor.web3.TransactionInstruction);
      
      // Verify plugin refs are included
      const numPluginRefs = instruction.data.readUInt16LE(6);
      expect(numPluginRefs).to.equal(2);
    });

    it('should build AddPlugin instruction', () => {
      const [walletAccount] = client.deriveWalletAccount(walletId);
      const pluginProgramId = anchor.web3.Keypair.generate().publicKey;
      const [pluginConfig] = client.derivePluginConfig(
        pluginProgramId,
        'role_permission_config',
        walletAccount
      );
      
      const params: AddPluginParams = {
        programId: pluginProgramId,
        configAccount: pluginConfig,
        pluginType: PluginType.RolePermission,
        enabled: true,
        priority: 0,
      };
      
      const instruction = client.buildAddPluginInstruction(
        walletId,
        params,
        payer.publicKey
      );
      
      expect(instruction).to.be.instanceOf(anchor.web3.TransactionInstruction);
      expect(instruction.programId.toBase58()).to.equal(LAZORKIT_V2_PROGRAM_ID.toBase58());
      expect(instruction.keys.length).to.equal(4);
      
      // Verify instruction data
      const discriminator = instruction.data.readUInt16LE(0);
      expect(discriminator).to.equal(3); // AddPlugin = 3
      
      // Verify plugin type
      const pluginType = instruction.data.readUInt8(66);
      expect(pluginType).to.equal(PluginType.RolePermission);
      
      // Verify enabled flag
      const enabled = instruction.data.readUInt8(67);
      expect(enabled).to.equal(1);
      
      // Verify priority
      const priority = instruction.data.readUInt8(68);
      expect(priority).to.equal(0);
    });

    it('should build Sign instruction with compact format', () => {
      const [walletAccount] = client.deriveWalletAccount(walletId);
      const [walletVault] = client.deriveWalletVault(walletAccount);
      
      // Create a simple transfer instruction
      const transferIx = anchor.web3.SystemProgram.transfer({
        fromPubkey: walletVault,
        toPubkey: anchor.web3.Keypair.generate().publicKey,
        lamports: 1000,
      });
      
      const params: SignParams = {
        authorityId: 0,
        instructions: [transferIx],
      };
      
      const instruction = client.buildSignInstruction(walletId, params);
      
      expect(instruction).to.be.instanceOf(anchor.web3.TransactionInstruction);
      expect(instruction.programId.toBase58()).to.equal(LAZORKIT_V2_PROGRAM_ID.toBase58());
      
      // Verify instruction data structure
      const discriminator = instruction.data.readUInt16LE(0);
      expect(discriminator).to.equal(1); // Sign = 1
      
      const payloadLen = instruction.data.readUInt16LE(2);
      expect(payloadLen).to.be.greaterThan(0);
      
      const authorityId = instruction.data.readUInt32LE(4);
      expect(authorityId).to.equal(0);
      
      // Verify compact instruction payload starts after header (8 bytes)
      const compactPayload = instruction.data.subarray(8, 8 + payloadLen);
      expect(compactPayload.length).to.equal(payloadLen);
      
      // Verify compact format: [num_instructions: u8, ...]
      const numInstructions = compactPayload.readUInt8(0);
      expect(numInstructions).to.equal(1);
    });

    it('should build Sign instruction with multiple inner instructions', () => {
      const [walletAccount] = client.deriveWalletAccount(walletId);
      const [walletVault] = client.deriveWalletVault(walletAccount);
      const recipient1 = anchor.web3.Keypair.generate().publicKey;
      const recipient2 = anchor.web3.Keypair.generate().publicKey;
      
      const transfer1 = anchor.web3.SystemProgram.transfer({
        fromPubkey: walletVault,
        toPubkey: recipient1,
        lamports: 1000,
      });
      
      const transfer2 = anchor.web3.SystemProgram.transfer({
        fromPubkey: walletVault,
        toPubkey: recipient2,
        lamports: 2000,
      });
      
      const params: SignParams = {
        authorityId: 0,
        instructions: [transfer1, transfer2],
      };
      
      const instruction = client.buildSignInstruction(walletId, params);
      
      expect(instruction).to.be.instanceOf(anchor.web3.TransactionInstruction);
      
      // Verify compact payload has 2 instructions
      const payloadLen = instruction.data.readUInt16LE(2);
      const compactPayload = instruction.data.subarray(8, 8 + payloadLen);
      const numInstructions = compactPayload.readUInt8(0);
      expect(numInstructions).to.equal(2);
    });

    it('should build Sign instruction with authority payload', () => {
      const [walletAccount] = client.deriveWalletAccount(walletId);
      const [walletVault] = client.deriveWalletVault(walletAccount);
      
      const transferIx = anchor.web3.SystemProgram.transfer({
        fromPubkey: walletVault,
        toPubkey: anchor.web3.Keypair.generate().publicKey,
        lamports: 1000,
      });
      
      const authorityPayload = Buffer.alloc(64); // Signature size
      crypto.getRandomValues(authorityPayload);
      
      const params: SignParams = {
        authorityId: 0,
        instructions: [transferIx],
        authorityPayload: authorityPayload,
      };
      
      const instruction = client.buildSignInstruction(walletId, params);
      
      expect(instruction).to.be.instanceOf(anchor.web3.TransactionInstruction);
      
      // Verify authority payload is included
      const payloadLen = instruction.data.readUInt16LE(2);
      const totalDataLen = instruction.data.length;
      const authorityPayloadStart = 8 + payloadLen;
      const authorityPayloadData = instruction.data.subarray(authorityPayloadStart);
      
      expect(authorityPayloadData.length).to.equal(authorityPayload.length);
      expect(authorityPayloadData).to.deep.equal(authorityPayload);
    });
  });

  describe('Edge Cases', () => {
    it('should handle empty wallet ID array', () => {
      const emptyId = new Uint8Array(32);
      emptyId.fill(0);
      
      const [walletAccount] = client.deriveWalletAccount(emptyId);
      expect(walletAccount).to.be.instanceOf(anchor.web3.PublicKey);
    });

    it('should handle AddAuthority with no plugin refs', () => {
      const authorityKeypair = anchor.web3.Keypair.generate();
      const authorityData = client.serializeEd25519Authority(authorityKeypair.publicKey);
      
      const params: AddAuthorityParams = {
        authorityType: AuthorityType.Ed25519,
        authorityData: authorityData,
        pluginRefs: [], // Empty plugin refs
      };
      
      const instruction = client.buildAddAuthorityInstruction(
        walletId,
        params,
        payer.publicKey
      );
      
      expect(instruction).to.be.instanceOf(anchor.web3.TransactionInstruction);
      const numPluginRefs = instruction.data.readUInt16LE(6);
      expect(numPluginRefs).to.equal(0);
    });

    it('should handle Sign instruction with empty instructions array', () => {
      const params: SignParams = {
        authorityId: 0,
        instructions: [],
      };
      
      // This should still build, but the compact payload will have num_instructions = 0
      const instruction = client.buildSignInstruction(walletId, params);
      
      expect(instruction).to.be.instanceOf(anchor.web3.TransactionInstruction);
      const payloadLen = instruction.data.readUInt16LE(2);
      const compactPayload = instruction.data.subarray(8, 8 + payloadLen);
      const numInstructions = compactPayload.readUInt8(0);
      expect(numInstructions).to.equal(0);
    });
  });

  describe('Account Ordering', () => {
    it('should order accounts correctly in Sign instruction', () => {
      const [walletAccount] = client.deriveWalletAccount(walletId);
      const [walletVault] = client.deriveWalletVault(walletAccount);
      const recipient = anchor.web3.Keypair.generate().publicKey;
      
      const transferIx = anchor.web3.SystemProgram.transfer({
        fromPubkey: walletVault,
        toPubkey: recipient,
        lamports: 1000,
      });
      
      const params: SignParams = {
        authorityId: 0,
        instructions: [transferIx],
      };
      
      const instruction = client.buildSignInstruction(walletId, params);
      
      // Verify account ordering:
      // 0: wallet_account (writable)
      // 1: wallet_vault (signer, not writable)
      // 2+: other accounts from inner instructions
      expect(instruction.keys.length).to.be.at.least(2);
      expect(instruction.keys[0].pubkey.toBase58()).to.equal(walletAccount.toBase58());
      expect(instruction.keys[0].isWritable).to.be.true;
      expect(instruction.keys[1].pubkey.toBase58()).to.equal(walletVault.toBase58());
      expect(instruction.keys[1].isSigner).to.be.true;
    });
  });
});
