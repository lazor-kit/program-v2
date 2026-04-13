/**
 * LazorKit Benchmark Script
 *
 * Measures real Compute Unit (CU) usage and transaction sizes for every
 * LazorKit instruction, compared against a normal SOL transfer baseline.
 *
 * Usage:
 *   1. Start local validator: npm run validator:start
 *   2. Run benchmarks:        npx tsx tests/benchmark.ts
 */
import {
  Connection,
  Keypair,
  LAMPORTS_PER_SOL,
  PublicKey,
  sendAndConfirmTransaction,
  SystemProgram,
  SYSVAR_INSTRUCTIONS_PUBKEY,
  Transaction,
  type TransactionInstruction,
  type Signer,
} from '@solana/web3.js';
import * as crypto from 'crypto';
import {
  findWalletPda,
  findVaultPda,
  findAuthorityPda,
  findSessionPda,
  createCreateWalletIx,
  createAddAuthorityIx,
  createExecuteIx,
  createCreateSessionIx,
  packCompactInstructions,
  computeAccountsHash,
  AUTH_TYPE_ED25519,
  AUTH_TYPE_SECP256R1,
  findDeferredExecPda,
  DISC_ADD_AUTHORITY,
  DISC_EXECUTE,
  DISC_AUTHORIZE,
  ROLE_ADMIN,
  PROGRAM_ID,
  createAuthorizeIx,
  createExecuteDeferredIx,
  computeInstructionsHash,
} from '../../sdk/solita-client/src';
import { generateMockSecp256r1Key, signSecp256r1 } from './secp256r1Utils';

const RPC_URL = process.env.RPC_URL || 'http://127.0.0.1:8899';

// ─── Helpers ──────────────────────────────────────────────────────────────────

interface BenchResult {
  name: string;
  cu: number;
  txSize: number;
  ixData: number;
  accounts: number;
  instructions: number;
}

async function setup() {
  const connection = new Connection(RPC_URL, 'confirmed');
  const payer = Keypair.generate();
  const sig = await connection.requestAirdrop(payer.publicKey, 100 * LAMPORTS_PER_SOL);
  await connection.confirmTransaction(sig, 'confirmed');
  return { connection, payer };
}

async function getSlot(connection: Connection): Promise<bigint> {
  const slot = await connection.getSlot('confirmed');
  return BigInt(slot);
}

async function sendAndMeasure(
  connection: Connection,
  payer: Keypair,
  instructions: TransactionInstruction[],
  extraSigners: Signer[] = [],
): Promise<{ cu: number; txSize: number; ixDataSizes: number[]; accountCounts: number[] }> {
  const tx = new Transaction();
  for (const ix of instructions) tx.add(ix);

  const { blockhash } = await connection.getLatestBlockhash('confirmed');
  tx.recentBlockhash = blockhash;
  tx.feePayer = payer.publicKey;

  const allSigners = [payer, ...extraSigners];
  tx.sign(...allSigners);
  const serialized = tx.serialize();
  const txSize = serialized.length;

  const sig = await sendAndConfirmTransaction(connection, tx, allSigners, {
    commitment: 'confirmed',
  });

  // Wait briefly then fetch transaction details for CU
  const txInfo = await connection.getTransaction(sig, {
    commitment: 'confirmed',
    maxSupportedTransactionVersion: 0,
  });

  const cu = txInfo?.meta?.computeUnitsConsumed ?? 0;
  const ixDataSizes = instructions.map(ix => ix.data.length);
  const accountCounts = instructions.map(ix => ix.keys.length);

  return { cu, txSize, ixDataSizes, accountCounts };
}

// ─── Benchmarks ───────────────────────────────────────────────────────────────

async function benchNormalTransfer(connection: Connection, payer: Keypair): Promise<BenchResult> {
  const recipient = Keypair.generate().publicKey;
  const ix = SystemProgram.transfer({
    fromPubkey: payer.publicKey,
    toPubkey: recipient,
    lamports: 1_000_000,
  });

  const result = await sendAndMeasure(connection, payer, [ix]);
  return {
    name: 'Normal SOL Transfer',
    cu: result.cu,
    txSize: result.txSize,
    ixData: result.ixDataSizes[0],
    accounts: result.accountCounts[0],
    instructions: 1,
  };
}

async function benchCreateWalletEd25519(connection: Connection, payer: Keypair): Promise<BenchResult> {
  const ownerKp = Keypair.generate();
  const userSeed = crypto.randomBytes(32);
  const pubkeyBytes = ownerKp.publicKey.toBytes();

  const [walletPda] = findWalletPda(userSeed);
  const [vaultPda] = findVaultPda(walletPda);
  const [authPda, authBump] = findAuthorityPda(walletPda, pubkeyBytes);

  const ix = createCreateWalletIx({
    payer: payer.publicKey,
    walletPda,
    vaultPda,
    authorityPda: authPda,
    userSeed,
    authType: AUTH_TYPE_ED25519,
    authBump,
    credentialOrPubkey: pubkeyBytes,
  });

  const result = await sendAndMeasure(connection, payer, [ix]);
  return {
    name: 'CreateWallet (Ed25519)',
    cu: result.cu,
    txSize: result.txSize,
    ixData: result.ixDataSizes[0],
    accounts: result.accountCounts[0],
    instructions: 1,
  };
}

async function benchCreateWalletSecp256r1(connection: Connection, payer: Keypair): Promise<BenchResult> {
  const key = await generateMockSecp256r1Key();
  const userSeed = crypto.randomBytes(32);

  const [walletPda] = findWalletPda(userSeed);
  const [vaultPda] = findVaultPda(walletPda);
  const [authPda, authBump] = findAuthorityPda(walletPda, key.credentialIdHash);

  const ix = createCreateWalletIx({
    payer: payer.publicKey,
    walletPda,
    vaultPda,
    authorityPda: authPda,
    userSeed,
    authType: AUTH_TYPE_SECP256R1,
    authBump,
    credentialOrPubkey: key.credentialIdHash,
    secp256r1Pubkey: key.publicKeyBytes,
    rpId: key.rpId,
  });

  const result = await sendAndMeasure(connection, payer, [ix]);
  return {
    name: 'CreateWallet (Secp256r1)',
    cu: result.cu,
    txSize: result.txSize,
    ixData: result.ixDataSizes[0],
    accounts: result.accountCounts[0],
    instructions: 1,
  };
}

async function benchAddAuthorityEd25519(connection: Connection, payer: Keypair): Promise<BenchResult> {
  // Setup: create wallet first
  const ownerKp = Keypair.generate();
  const userSeed = crypto.randomBytes(32);
  const pubkeyBytes = ownerKp.publicKey.toBytes();

  const [walletPda] = findWalletPda(userSeed);
  const [vaultPda] = findVaultPda(walletPda);
  const [authPda, authBump] = findAuthorityPda(walletPda, pubkeyBytes);

  await sendAndMeasure(connection, payer, [createCreateWalletIx({
    payer: payer.publicKey,
    walletPda,
    vaultPda,
    authorityPda: authPda,
    userSeed,
    authType: AUTH_TYPE_ED25519,
    authBump,
    credentialOrPubkey: pubkeyBytes,
  })]);

  // Now add a new Ed25519 authority (admin adds spender)
  const newKp = Keypair.generate();
  const newPubkey = newKp.publicKey.toBytes();
  const [newAuthPda] = findAuthorityPda(walletPda, newPubkey);

  const ix = createAddAuthorityIx({
    payer: payer.publicKey,
    walletPda,
    adminAuthorityPda: authPda,
    newAuthorityPda: newAuthPda,
    newType: AUTH_TYPE_ED25519,
    newRole: ROLE_ADMIN,
    credentialOrPubkey: newPubkey,
    authorizerSigner: ownerKp.publicKey,
  });

  const result = await sendAndMeasure(connection, payer, [ix], [ownerKp]);
  return {
    name: 'AddAuthority (Ed25519 admin)',
    cu: result.cu,
    txSize: result.txSize,
    ixData: result.ixDataSizes[0],
    accounts: result.accountCounts[0],
    instructions: 1,
  };
}

async function benchExecuteSecp256r1(connection: Connection, payer: Keypair): Promise<BenchResult> {
  // Setup: create Secp256r1 wallet
  const key = await generateMockSecp256r1Key();
  const userSeed = crypto.randomBytes(32);

  const [walletPda] = findWalletPda(userSeed);
  const [vaultPda] = findVaultPda(walletPda);
  const [authPda, authBump] = findAuthorityPda(walletPda, key.credentialIdHash);

  await sendAndMeasure(connection, payer, [createCreateWalletIx({
    payer: payer.publicKey,
    walletPda,
    vaultPda,
    authorityPda: authPda,
    userSeed,
    authType: AUTH_TYPE_SECP256R1,
    authBump,
    credentialOrPubkey: key.credentialIdHash,
    secp256r1Pubkey: key.publicKeyBytes,
    rpId: key.rpId,
  })]);

  // Fund vault
  const airdropSig = await connection.requestAirdrop(vaultPda, 5 * LAMPORTS_PER_SOL);
  await connection.confirmTransaction(airdropSig, 'confirmed');

  const recipient = Keypair.generate().publicKey;
  const slot = await getSlot(connection);

  // Build SOL transfer compact instruction
  const transferData = Buffer.alloc(12);
  transferData.writeUInt32LE(2, 0); // Transfer discriminator
  transferData.writeBigUInt64LE(1_000_000n, 4);

  // Account layout (no slotHashes sysvar):
  //   0: payer, 1: wallet, 2: authority, 3: vault
  //   4: sysvar_instructions
  //   5: SystemProgram, 6: recipient
  const compactIxs = [{
    programIdIndex: 5,       // SystemProgram at index 5
    accountIndexes: [3, 6],  // vault, recipient
    data: new Uint8Array(transferData),
  }];
  const packed = packCompactInstructions(compactIxs);

  // Compute accounts hash for signature binding
  const allAccountMetas = [
    { pubkey: payer.publicKey, isSigner: true, isWritable: false },
    { pubkey: walletPda, isSigner: false, isWritable: false },
    { pubkey: authPda, isSigner: false, isWritable: true },
    { pubkey: vaultPda, isSigner: false, isWritable: true },
    { pubkey: SYSVAR_INSTRUCTIONS_PUBKEY, isSigner: false, isWritable: false },
    { pubkey: SystemProgram.programId, isSigner: false, isWritable: false },
    { pubkey: recipient, isSigner: false, isWritable: true },
  ];
  const accountsHash = computeAccountsHash(allAccountMetas, compactIxs);
  const signedPayload = Buffer.concat([packed, accountsHash]);

  const { authPayload, precompileIx } = await signSecp256r1({
    key,
    discriminator: new Uint8Array([DISC_EXECUTE]),
    signedPayload,
    slot,
    counter: 1,
    payer: payer.publicKey,
    sysvarIxIndex: 4,
  });

  const ix = createExecuteIx({
    payer: payer.publicKey,
    walletPda,
    authorityPda: authPda,
    vaultPda,
    packedInstructions: packed,
    authPayload,
    remainingAccounts: [
      { pubkey: SystemProgram.programId, isSigner: false, isWritable: false },
      { pubkey: recipient, isSigner: false, isWritable: true },
    ],
  });

  const result = await sendAndMeasure(connection, payer, [precompileIx, ix]);
  return {
    name: 'Execute Secp256r1 (SOL transfer)',
    cu: result.cu,
    txSize: result.txSize,
    ixData: result.ixDataSizes.reduce((a, b) => a + b, 0),
    accounts: result.accountCounts[result.accountCounts.length - 1], // Execute ix accounts
    instructions: 2,
  };
}

async function benchCreateSession(connection: Connection, payer: Keypair): Promise<BenchResult> {
  // Setup: create Ed25519 wallet
  const ownerKp = Keypair.generate();
  const userSeed = crypto.randomBytes(32);
  const pubkeyBytes = ownerKp.publicKey.toBytes();

  const [walletPda] = findWalletPda(userSeed);
  const [vaultPda] = findVaultPda(walletPda);
  const [authPda, authBump] = findAuthorityPda(walletPda, pubkeyBytes);

  await sendAndMeasure(connection, payer, [createCreateWalletIx({
    payer: payer.publicKey,
    walletPda,
    vaultPda,
    authorityPda: authPda,
    userSeed,
    authType: AUTH_TYPE_ED25519,
    authBump,
    credentialOrPubkey: pubkeyBytes,
  })]);

  const sessionKp = Keypair.generate();
  const sessionKeyBytes = sessionKp.publicKey.toBytes();
  const [sessionPda] = findSessionPda(walletPda, sessionKeyBytes);

  const currentSlot = await getSlot(connection);
  const expiresAt = currentSlot + 9000n;

  const ix = createCreateSessionIx({
    payer: payer.publicKey,
    walletPda,
    adminAuthorityPda: authPda,
    sessionPda,
    sessionKey: sessionKeyBytes,
    expiresAt,
    authorizerSigner: ownerKp.publicKey,
  });

  const result = await sendAndMeasure(connection, payer, [ix], [ownerKp]);
  return {
    name: 'CreateSession (Ed25519)',
    cu: result.cu,
    txSize: result.txSize,
    ixData: result.ixDataSizes[0],
    accounts: result.accountCounts[0],
    instructions: 1,
  };
}

async function benchExecuteSession(connection: Connection, payer: Keypair): Promise<BenchResult> {
  // Setup: create Ed25519 wallet + session
  const ownerKp = Keypair.generate();
  const userSeed = crypto.randomBytes(32);
  const pubkeyBytes = ownerKp.publicKey.toBytes();

  const [walletPda] = findWalletPda(userSeed);
  const [vaultPda] = findVaultPda(walletPda);
  const [authPda, authBump] = findAuthorityPda(walletPda, pubkeyBytes);

  await sendAndMeasure(connection, payer, [createCreateWalletIx({
    payer: payer.publicKey,
    walletPda,
    vaultPda,
    authorityPda: authPda,
    userSeed,
    authType: AUTH_TYPE_ED25519,
    authBump,
    credentialOrPubkey: pubkeyBytes,
  })]);

  // Create session
  const sessionKp = Keypair.generate();
  const sessionKeyBytes = sessionKp.publicKey.toBytes();
  const [sessionPda] = findSessionPda(walletPda, sessionKeyBytes);

  const currentSlot = await getSlot(connection);
  const expiresAt = currentSlot + 9000n;

  await sendAndMeasure(connection, payer, [createCreateSessionIx({
    payer: payer.publicKey,
    walletPda,
    adminAuthorityPda: authPda,
    sessionPda,
    sessionKey: sessionKeyBytes,
    expiresAt,
    authorizerSigner: ownerKp.publicKey,
  })], [ownerKp]);

  // Fund vault
  const airdropSig = await connection.requestAirdrop(vaultPda, 5 * LAMPORTS_PER_SOL);
  await connection.confirmTransaction(airdropSig, 'confirmed');

  // Build SOL transfer compact instruction
  // Account layout (no sysvars needed for session auth):
  //   0: payer, 1: wallet, 2: sessionPda (as authority), 3: vault
  //   4: sessionKey (signer), 5: SystemProgram, 6: recipient
  const recipient = Keypair.generate().publicKey;
  const transferData = Buffer.alloc(12);
  transferData.writeUInt32LE(2, 0); // Transfer discriminator
  transferData.writeBigUInt64LE(1_000_000n, 4);

  const packed = packCompactInstructions([{
    programIdIndex: 5,       // SystemProgram at index 5
    accountIndexes: [3, 6],  // vault, recipient
    data: new Uint8Array(transferData),
  }]);

  const ix = createExecuteIx({
    payer: payer.publicKey,
    walletPda,
    authorityPda: sessionPda,  // Session PDA as authority
    vaultPda,
    packedInstructions: packed,
    // No authPayload — session uses signer-based auth
    remainingAccounts: [
      { pubkey: sessionKp.publicKey, isSigner: true, isWritable: false },
      { pubkey: SystemProgram.programId, isSigner: false, isWritable: false },
      { pubkey: recipient, isSigner: false, isWritable: true },
    ],
  });

  const result = await sendAndMeasure(connection, payer, [ix], [sessionKp]);
  return {
    name: 'Execute Session (SOL transfer)',
    cu: result.cu,
    txSize: result.txSize,
    ixData: result.ixDataSizes[0],
    accounts: result.accountCounts[0],
    instructions: 1,
  };
}

// ─── Deferred Execution Benchmarks ───────────────────────────────────────────

async function benchDeferredExecution(
  connection: Connection,
  payer: Keypair,
): Promise<{ authorize: BenchResult; executeDeferred: BenchResult }> {
  // Setup: create Secp256r1 wallet
  const key = await generateMockSecp256r1Key();
  const userSeed = crypto.randomBytes(32);

  const [walletPda] = findWalletPda(userSeed);
  const [vaultPda] = findVaultPda(walletPda);
  const [authPda, authBump] = findAuthorityPda(walletPda, key.credentialIdHash);

  await sendAndMeasure(connection, payer, [createCreateWalletIx({
    payer: payer.publicKey,
    walletPda,
    vaultPda,
    authorityPda: authPda,
    userSeed,
    authType: AUTH_TYPE_SECP256R1,
    authBump,
    credentialOrPubkey: key.credentialIdHash,
    secp256r1Pubkey: key.publicKeyBytes,
    rpId: key.rpId,
  })]);

  // Fund vault
  const airdropSig = await connection.requestAirdrop(vaultPda, 10 * LAMPORTS_PER_SOL);
  await connection.confirmTransaction(airdropSig, 'confirmed');

  const recipient = Keypair.generate().publicKey;
  const slot = await getSlot(connection);

  // Build SOL transfer as compact instruction
  const transferData = Buffer.alloc(12);
  transferData.writeUInt32LE(2, 0);
  transferData.writeBigUInt64LE(BigInt(LAMPORTS_PER_SOL), 4);

  // TX2 account layout:
  //   0: payer, 1: wallet, 2: vault, 3: deferred, 4: refundDest
  //   5: SystemProgram, 6: recipient
  const compactIxs = [{
    programIdIndex: 5,
    accountIndexes: [2, 6],
    data: new Uint8Array(transferData),
  }];

  // Compute hashes
  const instructionsHash = computeInstructionsHash(compactIxs);
  const tx2AccountMetas = [
    { pubkey: payer.publicKey, isSigner: true, isWritable: true },
    { pubkey: walletPda, isSigner: false, isWritable: false },
    { pubkey: vaultPda, isSigner: false, isWritable: true },
    { pubkey: PublicKey.default, isSigner: false, isWritable: true }, // deferred placeholder
    { pubkey: payer.publicKey, isSigner: false, isWritable: true },
    { pubkey: SystemProgram.programId, isSigner: false, isWritable: false },
    { pubkey: recipient, isSigner: false, isWritable: true },
  ];
  const accountsHash = computeAccountsHash(tx2AccountMetas, compactIxs);

  const signedPayload = Buffer.concat([instructionsHash, accountsHash]);

  const { authPayload, precompileIx } = await signSecp256r1({
    key,
    discriminator: new Uint8Array([DISC_AUTHORIZE]),
    signedPayload,
    slot,
    counter: 1,
    payer: payer.publicKey,
    sysvarIxIndex: 6,
  });

  const [deferredExecPda] = findDeferredExecPda(walletPda, authPda, 1);

  // === TX1: Authorize ===
  const authorizeIx = createAuthorizeIx({
    payer: payer.publicKey,
    walletPda,
    authorityPda: authPda,
    deferredExecPda,
    instructionsHash,
    accountsHash,
    expiryOffset: 300,
    authPayload,
  });

  const authResult = await sendAndMeasure(connection, payer, [precompileIx, authorizeIx]);
  const authorizeResult: BenchResult = {
    name: 'Deferred: Authorize (TX1)',
    cu: authResult.cu,
    txSize: authResult.txSize,
    ixData: authResult.ixDataSizes.reduce((a, b) => a + b, 0),
    accounts: authResult.accountCounts[authResult.accountCounts.length - 1],
    instructions: 2,
  };

  // === TX2: ExecuteDeferred ===
  const packed = packCompactInstructions(compactIxs);
  const executeDeferredIx = createExecuteDeferredIx({
    payer: payer.publicKey,
    walletPda,
    vaultPda,
    deferredExecPda,
    refundDestination: payer.publicKey,
    packedInstructions: packed,
    remainingAccounts: [
      { pubkey: SystemProgram.programId, isSigner: false, isWritable: false },
      { pubkey: recipient, isSigner: false, isWritable: true },
    ],
  });

  const execResult = await sendAndMeasure(connection, payer, [executeDeferredIx]);
  const executeDeferredResult: BenchResult = {
    name: 'Deferred: ExecuteDeferred (TX2)',
    cu: execResult.cu,
    txSize: execResult.txSize,
    ixData: execResult.ixDataSizes[0],
    accounts: execResult.accountCounts[0],
    instructions: 1,
  };

  return { authorize: authorizeResult, executeDeferred: executeDeferredResult };
}

async function benchDeferredMultiInstruction(
  connection: Connection,
  payer: Keypair,
): Promise<{ authorize: BenchResult; executeDeferred: BenchResult }> {
  // Setup: create Secp256r1 wallet
  const key = await generateMockSecp256r1Key();
  const userSeed = crypto.randomBytes(32);

  const [walletPda] = findWalletPda(userSeed);
  const [vaultPda] = findVaultPda(walletPda);
  const [authPda, authBump] = findAuthorityPda(walletPda, key.credentialIdHash);

  await sendAndMeasure(connection, payer, [createCreateWalletIx({
    payer: payer.publicKey,
    walletPda,
    vaultPda,
    authorityPda: authPda,
    userSeed,
    authType: AUTH_TYPE_SECP256R1,
    authBump,
    credentialOrPubkey: key.credentialIdHash,
    secp256r1Pubkey: key.publicKeyBytes,
    rpId: key.rpId,
  })]);

  // Fund vault
  const airdropSig = await connection.requestAirdrop(vaultPda, 10 * LAMPORTS_PER_SOL);
  await connection.confirmTransaction(airdropSig, 'confirmed');

  const recipient1 = Keypair.generate().publicKey;
  const recipient2 = Keypair.generate().publicKey;
  const recipient3 = Keypair.generate().publicKey;
  const slot = await getSlot(connection);

  const makeTransferData = (amount: bigint) => {
    const buf = Buffer.alloc(12);
    buf.writeUInt32LE(2, 0);
    buf.writeBigUInt64LE(amount, 4);
    return new Uint8Array(buf);
  };

  // TX2 layout:
  //   0: payer, 1: wallet, 2: vault, 3: deferred, 4: refundDest
  //   5: SystemProgram, 6: recipient1, 7: recipient2, 8: recipient3
  const compactIxs = [
    { programIdIndex: 5, accountIndexes: [2, 6], data: makeTransferData(BigInt(LAMPORTS_PER_SOL)) },
    { programIdIndex: 5, accountIndexes: [2, 7], data: makeTransferData(BigInt(LAMPORTS_PER_SOL)) },
    { programIdIndex: 5, accountIndexes: [2, 8], data: makeTransferData(BigInt(LAMPORTS_PER_SOL)) },
  ];

  const instructionsHash = computeInstructionsHash(compactIxs);
  const tx2AccountMetas = [
    { pubkey: payer.publicKey, isSigner: true, isWritable: true },
    { pubkey: walletPda, isSigner: false, isWritable: false },
    { pubkey: vaultPda, isSigner: false, isWritable: true },
    { pubkey: PublicKey.default, isSigner: false, isWritable: true },
    { pubkey: payer.publicKey, isSigner: false, isWritable: true },
    { pubkey: SystemProgram.programId, isSigner: false, isWritable: false },
    { pubkey: recipient1, isSigner: false, isWritable: true },
    { pubkey: recipient2, isSigner: false, isWritable: true },
    { pubkey: recipient3, isSigner: false, isWritable: true },
  ];
  const accountsHash = computeAccountsHash(tx2AccountMetas, compactIxs);
  const signedPayload = Buffer.concat([instructionsHash, accountsHash]);

  const { authPayload, precompileIx } = await signSecp256r1({
    key,
    discriminator: new Uint8Array([DISC_AUTHORIZE]),
    signedPayload,
    slot,
    counter: 1,
    payer: payer.publicKey,
    sysvarIxIndex: 6,
  });

  const [deferredExecPda] = findDeferredExecPda(walletPda, authPda, 1);

  // TX1: Authorize
  const authorizeIx = createAuthorizeIx({
    payer: payer.publicKey,
    walletPda,
    authorityPda: authPda,
    deferredExecPda,
    instructionsHash,
    accountsHash,
    expiryOffset: 300,
    authPayload,
  });

  const authResult = await sendAndMeasure(connection, payer, [precompileIx, authorizeIx]);
  const authorizeResult: BenchResult = {
    name: 'Deferred Multi-IX: Authorize (TX1)',
    cu: authResult.cu,
    txSize: authResult.txSize,
    ixData: authResult.ixDataSizes.reduce((a, b) => a + b, 0),
    accounts: authResult.accountCounts[authResult.accountCounts.length - 1],
    instructions: 2,
  };

  // TX2: ExecuteDeferred (3 transfers)
  const packed = packCompactInstructions(compactIxs);
  const executeDeferredIx = createExecuteDeferredIx({
    payer: payer.publicKey,
    walletPda,
    vaultPda,
    deferredExecPda,
    refundDestination: payer.publicKey,
    packedInstructions: packed,
    remainingAccounts: [
      { pubkey: SystemProgram.programId, isSigner: false, isWritable: false },
      { pubkey: recipient1, isSigner: false, isWritable: true },
      { pubkey: recipient2, isSigner: false, isWritable: true },
      { pubkey: recipient3, isSigner: false, isWritable: true },
    ],
  });

  const execResult = await sendAndMeasure(connection, payer, [executeDeferredIx]);
  const executeDeferredResult: BenchResult = {
    name: 'Deferred Multi-IX: ExecuteDeferred (TX2)',
    cu: execResult.cu,
    txSize: execResult.txSize,
    ixData: execResult.ixDataSizes[0],
    accounts: execResult.accountCounts[0],
    instructions: 1,
  };

  return { authorize: authorizeResult, executeDeferred: executeDeferredResult };
}

// ─── Rent Calculations ────────────────────────────────────────────────────────

function calculateRent(dataSize: number): number {
  // Solana rent formula: (128 + dataSize) * 6.96 SOL/byte/year * 2 years exemption
  // Actual formula: (128 + dataSize) * 3480 / 1_000_000_000 * 2 (approx)
  // More precisely, we use the known constant: 1 byte-year costs 0.00000348 SOL
  // Rent-exempt = (128 + dataSize) * 0.00000348 * 2
  const LAMPORTS_PER_BYTE_YEAR = 3480;
  const EXEMPTION_YEARS = 2;
  return (128 + dataSize) * LAMPORTS_PER_BYTE_YEAR * EXEMPTION_YEARS;
}

// ─── Main ─────────────────────────────────────────────────────────────────────

async function main() {
  console.log('LazorKit Benchmark Suite');
  console.log('========================\n');
  console.log(`RPC: ${RPC_URL}\n`);

  const { connection, payer } = await setup();

  const results: BenchResult[] = [];

  // Run benchmarks sequentially
  console.log('Running benchmarks...\n');

  const benchmarks = [
    { name: 'Normal SOL Transfer', fn: () => benchNormalTransfer(connection, payer) },
    { name: 'CreateWallet Ed25519', fn: () => benchCreateWalletEd25519(connection, payer) },
    { name: 'CreateWallet Secp256r1', fn: () => benchCreateWalletSecp256r1(connection, payer) },
    { name: 'AddAuthority Ed25519', fn: () => benchAddAuthorityEd25519(connection, payer) },
    { name: 'Execute Secp256r1', fn: () => benchExecuteSecp256r1(connection, payer) },
    { name: 'Execute Session', fn: () => benchExecuteSession(connection, payer) },
    { name: 'CreateSession', fn: () => benchCreateSession(connection, payer) },
  ];

  for (const bench of benchmarks) {
    try {
      process.stdout.write(`  ${bench.name}... `);
      const result = await bench.fn();
      results.push(result);
      console.log(`${result.cu} CU, ${result.txSize} bytes`);
    } catch (err: any) {
      console.log(`FAILED: ${err.message?.slice(0, 80)}`);
    }
  }

  // Run deferred execution benchmarks (these return 2 results each)
  let deferredSingle: { authorize: BenchResult; executeDeferred: BenchResult } | null = null;
  let deferredMulti: { authorize: BenchResult; executeDeferred: BenchResult } | null = null;

  try {
    process.stdout.write('  Deferred Single (Authorize + Execute)... ');
    deferredSingle = await benchDeferredExecution(connection, payer);
    results.push(deferredSingle.authorize);
    results.push(deferredSingle.executeDeferred);
    console.log(`TX1: ${deferredSingle.authorize.cu} CU, TX2: ${deferredSingle.executeDeferred.cu} CU`);
  } catch (err: any) {
    console.log(`FAILED: ${err.message?.slice(0, 80)}`);
  }

  try {
    process.stdout.write('  Deferred Multi-IX (3 transfers)... ');
    deferredMulti = await benchDeferredMultiInstruction(connection, payer);
    results.push(deferredMulti.authorize);
    results.push(deferredMulti.executeDeferred);
    console.log(`TX1: ${deferredMulti.authorize.cu} CU, TX2: ${deferredMulti.executeDeferred.cu} CU`);
  } catch (err: any) {
    console.log(`FAILED: ${err.message?.slice(0, 80)}`);
  }

  // ─── Output Tables ────────────────────────────────────────────────────────

  console.log('\n\n## Compute Units & Transaction Size\n');
  console.log('| Instruction | CU | Tx Size (bytes) | Ix Data (bytes) | Accounts | Instructions |');
  console.log('|---|---|---|---|---|---|');
  for (const r of results) {
    console.log(`| ${r.name} | ${r.cu.toLocaleString()} | ${r.txSize} | ${r.ixData} | ${r.accounts} | ${r.instructions} |`);
  }

  // ─── Comparison Table ─────────────────────────────────────────────────────

  const normalResult = results.find(r => r.name === 'Normal SOL Transfer');
  const secp256r1Result = results.find(r => r.name === 'Execute Secp256r1 (SOL transfer)');
  const sessionResult = results.find(r => r.name === 'Execute Session (SOL transfer)');

  if (normalResult && secp256r1Result) {
    console.log('\n\n## LazorKit vs Normal SOL Transfer\n');
    console.log('| Metric | Normal Transfer | LazorKit Secp256r1 | LazorKit Session | Notes |');
    console.log('|---|---|---|---|---|');
    console.log(`| Compute Units | ${normalResult.cu.toLocaleString()} | ${secp256r1Result.cu.toLocaleString()} | ${sessionResult ? sessionResult.cu.toLocaleString() : 'N/A'} | Session uses Ed25519 signer (no precompile) |`);
    console.log(`| Transaction Size | ${normalResult.txSize} bytes | ${secp256r1Result.txSize} bytes | ${sessionResult ? sessionResult.txSize + ' bytes' : 'N/A'} | Session tx is smaller (no precompile ix) |`);
    console.log(`| Instruction Data | ${normalResult.ixData} bytes | ${secp256r1Result.ixData} bytes | ${sessionResult ? sessionResult.ixData + ' bytes' : 'N/A'} | Session has no auth payload |`);
    console.log(`| Accounts | ${normalResult.accounts} | ${secp256r1Result.accounts} | ${sessionResult ? sessionResult.accounts : 'N/A'} | Session skips sysvar accounts |`);
    console.log(`| Instructions per Tx | ${normalResult.instructions} | ${secp256r1Result.instructions} | ${sessionResult ? sessionResult.instructions : 'N/A'} | Session needs only 1 ix |`);
    console.log(`| Transaction Fee | 0.000005 SOL | 0.000005 SOL | 0.000005 SOL | Same base fee |`);
  }

  // ─── Deferred vs Immediate Execute Comparison ─────────────────────────────

  if (secp256r1Result && deferredSingle) {
    const totalDeferredCU = deferredSingle.authorize.cu + deferredSingle.executeDeferred.cu;
    const deferredRent = calculateRent(176);

    console.log('\n\n## Deferred Execution vs Immediate Execute (Secp256r1)\n');
    console.log('| Metric | Immediate Execute | Deferred TX1 (Authorize) | Deferred TX2 (Execute) | Deferred Total | Notes |');
    console.log('|---|---|---|---|---|---|');
    console.log(`| Compute Units | ${secp256r1Result.cu.toLocaleString()} | ${deferredSingle.authorize.cu.toLocaleString()} | ${deferredSingle.executeDeferred.cu.toLocaleString()} | ${totalDeferredCU.toLocaleString()} | Deferred splits CU across 2 txs |`);
    console.log(`| Tx Size (bytes) | ${secp256r1Result.txSize} | ${deferredSingle.authorize.txSize} | ${deferredSingle.executeDeferred.txSize} | ${deferredSingle.authorize.txSize + deferredSingle.executeDeferred.txSize} | TX2 has no precompile overhead |`);
    console.log(`| Inner Ix Capacity | ~574 bytes | N/A (hashes only) | ~1,100 bytes | ~1,100 bytes | 1.9x more space for inner instructions |`);
    console.log(`| Tx Fees | 0.000005 SOL | 0.000005 SOL | 0.000005 SOL | 0.00001 SOL | 2x fee for 2 transactions |`);
    console.log(`| Temp Rent (refunded) | — | ${(deferredRent / LAMPORTS_PER_SOL).toFixed(9)} SOL | refunded | 0 SOL net | DeferredExec rent refunded on close |`);
    console.log(`| Accounts | ${secp256r1Result.accounts} | ${deferredSingle.authorize.accounts} | ${deferredSingle.executeDeferred.accounts} | — | TX2 doesn't need precompile sysvar |`);
  }

  if (deferredMulti) {
    console.log('\n\n## Deferred Multi-Instruction (3 SOL Transfers)\n');
    console.log('| Phase | CU | Tx Size (bytes) | Accounts | Notes |');
    console.log('|---|---|---|---|---|');
    console.log(`| TX1: Authorize | ${deferredMulti.authorize.cu.toLocaleString()} | ${deferredMulti.authorize.txSize} | ${deferredMulti.authorize.accounts} | Same cost regardless of inner ix count |`);
    console.log(`| TX2: Execute 3 transfers | ${deferredMulti.executeDeferred.cu.toLocaleString()} | ${deferredMulti.executeDeferred.txSize} | ${deferredMulti.executeDeferred.accounts} | 3 CPIs + hash verification |`);
    console.log(`| Total | ${(deferredMulti.authorize.cu + deferredMulti.executeDeferred.cu).toLocaleString()} | ${deferredMulti.authorize.txSize + deferredMulti.executeDeferred.txSize} | — | |`);
  }

  // ─── Rent Costs ───────────────────────────────────────────────────────────

  const rentData = [
    { name: 'Wallet PDA', dataSize: 8 },
    { name: 'Authority (Ed25519)', dataSize: 80 },
    { name: 'Authority (Secp256r1)', dataSize: 125 }, // 48 header + 32 cred_hash + 33 pubkey + 1 rpIdLen + ~11 rpId
    { name: 'Session', dataSize: 80 },
    { name: 'DeferredExec (temporary)', dataSize: 176 },
  ];

  console.log('\n\n## Rent-Exempt Costs\n');
  console.log('| Account | Data (bytes) | Rent-Exempt (SOL) | Rent-Exempt (lamports) |');
  console.log('|---|---|---|---|');
  for (const r of rentData) {
    const lamports = calculateRent(r.dataSize);
    console.log(`| ${r.name} | ${r.dataSize} | ${(lamports / LAMPORTS_PER_SOL).toFixed(9)} | ${lamports.toLocaleString()} |`);
  }

  // Total wallet creation cost
  const walletRent = calculateRent(8);
  const authEd25519Rent = calculateRent(80);
  const authSecp256r1Rent = calculateRent(125);
  const sessionRent = calculateRent(80);
  const txFee = 5000; // 0.000005 SOL

  console.log('\n\n## Total Wallet Creation Cost\n');
  console.log('| Auth Type | Wallet Rent | Authority Rent | Tx Fee | Total |');
  console.log('|---|---|---|---|---|');
  const totalEd25519 = walletRent + authEd25519Rent + txFee;
  const totalSecp256r1 = walletRent + authSecp256r1Rent + txFee;
  console.log(`| Ed25519 | ${(walletRent / LAMPORTS_PER_SOL).toFixed(9)} | ${(authEd25519Rent / LAMPORTS_PER_SOL).toFixed(9)} | ${(txFee / LAMPORTS_PER_SOL).toFixed(9)} | ${(totalEd25519 / LAMPORTS_PER_SOL).toFixed(9)} SOL |`);
  console.log(`| Secp256r1 | ${(walletRent / LAMPORTS_PER_SOL).toFixed(9)} | ${(authSecp256r1Rent / LAMPORTS_PER_SOL).toFixed(9)} | ${(txFee / LAMPORTS_PER_SOL).toFixed(9)} | ${(totalSecp256r1 / LAMPORTS_PER_SOL).toFixed(9)} SOL |`);

  // Session cost
  console.log('\n\n## Session Key Cost\n');
  console.log('| Item | Cost |');
  console.log('|---|---|');
  console.log(`| Session account rent | ${(sessionRent / LAMPORTS_PER_SOL).toFixed(9)} SOL |`);
  console.log(`| CreateSession tx fee | ${(txFee / LAMPORTS_PER_SOL).toFixed(9)} SOL |`);
  console.log(`| Total (create session) | ${((sessionRent + txFee) / LAMPORTS_PER_SOL).toFixed(9)} SOL |`);
  console.log(`| Execute via session tx fee | ${(txFee / LAMPORTS_PER_SOL).toFixed(9)} SOL |`);
  console.log('');
  console.log('Session rent is refundable after expiry. Execute via session costs only the base tx fee.');

  console.log('\n\nBenchmark complete.');
}

main().catch((err) => {
  console.error('Benchmark failed:', err);
  process.exit(1);
});
