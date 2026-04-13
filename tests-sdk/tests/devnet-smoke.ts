/**
 * Comprehensive devnet smoke test — exercises ALL instructions, auth types, and roles.
 * Reports actual CU consumption and rent costs for every operation.
 *
 * Run: npx tsx tests/devnet-smoke.ts
 */
import {
  Connection,
  Keypair,
  LAMPORTS_PER_SOL,
  SystemProgram,
  Transaction,
  sendAndConfirmTransaction,
  type TransactionInstruction,
  type Signer,
} from '@solana/web3.js';
import * as crypto from 'crypto';
import * as fs from 'fs';
import * as path from 'path';
import {
  LazorKitClient,
  ed25519,
  secp256r1,
  session,
  ROLE_ADMIN,
  ROLE_SPENDER,
} from '../../sdk/solita-client/src';
import { generateMockSecp256r1Key, createMockSigner } from './secp256r1Utils';

const RPC_URL = process.env.RPC_URL || 'https://api.devnet.solana.com';

// ─── Helpers ──────────────────────────────────────────────────────────

interface TxResult {
  sig: string;
  cu: number;
  txSize: number;
  rentCost?: number;
}

async function loadPayer(): Promise<Keypair> {
  const keypairPath = path.resolve(process.env.HOME || '~', '.config/solana/id.json');
  const raw = JSON.parse(fs.readFileSync(keypairPath, 'utf-8'));
  return Keypair.fromSecretKey(new Uint8Array(raw));
}

async function sendAndMeasure(
  connection: Connection,
  payer: Keypair,
  instructions: TransactionInstruction[],
  extraSigners: Signer[] = [],
): Promise<TxResult> {
  const tx = new Transaction();
  for (const ix of instructions) tx.add(ix);

  const { blockhash } = await connection.getLatestBlockhash('confirmed');
  tx.recentBlockhash = blockhash;
  tx.feePayer = payer.publicKey;
  const allSigners = [payer, ...extraSigners];
  tx.sign(...allSigners);
  const txSize = tx.serialize().length;

  const sig = await sendAndConfirmTransaction(connection, tx, allSigners, { commitment: 'confirmed' });

  const txInfo = await connection.getTransaction(sig, {
    commitment: 'confirmed',
    maxSupportedTransactionVersion: 0,
  });
  const cu = txInfo?.meta?.computeUnitsConsumed ?? 0;

  return { sig, cu, txSize };
}

function printRow(label: string, result: TxResult, extra = '') {
  const cuStr = result.cu.toLocaleString().padStart(8);
  const sizeStr = `${result.txSize}`.padStart(5);
  const rentStr = result.rentCost !== undefined
    ? `${(result.rentCost / LAMPORTS_PER_SOL).toFixed(6)} SOL`
    : '-';
  console.log(
    `  ${label.padEnd(52)} ${cuStr} CU  ${sizeStr} bytes  rent: ${rentStr}  ${extra}`,
  );
}

const results: { label: string; result: TxResult }[] = [];

function record(label: string, result: TxResult) {
  results.push({ label, result });
  printRow(label, result);
}

// ─── Main ─────────────────────────────────────────────────────────────

async function main() {
  const connection = new Connection(RPC_URL, 'confirmed');
  const payer = await loadPayer();
  const client = new LazorKitClient(connection);

  const payerBalance = await connection.getBalance(payer.publicKey);
  console.log(`Payer:   ${payer.publicKey.toBase58()}`);
  console.log(`Balance: ${(payerBalance / LAMPORTS_PER_SOL).toFixed(4)} SOL`);
  console.log(`RPC:     ${RPC_URL}\n`);

  console.log('=' .repeat(100));
  console.log('  OPERATION'.padEnd(54) + '      CU   SIZE  RENT');
  console.log('=' .repeat(100));

  // ────────────────────────────────────────────────────────────
  // 1. BASELINE: Normal SOL transfer
  // ────────────────────────────────────────────────────────────
  console.log('\n--- Baseline ---');
  {
    const recipient = Keypair.generate().publicKey;
    const ix = SystemProgram.transfer({ fromPubkey: payer.publicKey, toPubkey: recipient, lamports: 1_000_000 });
    const r = await sendAndMeasure(connection, payer, [ix]);
    record('Normal SOL Transfer (baseline)', r);
  }

  // ────────────────────────────────────────────────────────────
  // 2. CREATE WALLET — Ed25519 owner
  // ────────────────────────────────────────────────────────────
  console.log('\n--- CreateWallet ---');
  let ed25519OwnerKp: Keypair;
  let ed25519WalletPda: any, ed25519VaultPda: any, ed25519OwnerAuthPda: any;
  {
    ed25519OwnerKp = Keypair.generate();
    const balBefore = await connection.getBalance(payer.publicKey);
    const { instructions, walletPda, vaultPda, authorityPda } = client.createWallet({
      payer: payer.publicKey,
      userSeed: crypto.randomBytes(32),
      owner: { type: 'ed25519', publicKey: ed25519OwnerKp.publicKey },
    });
    const r = await sendAndMeasure(connection, payer, instructions);
    const balAfter = await connection.getBalance(payer.publicKey);
    r.rentCost = balBefore - balAfter - 5000; // subtract tx fee
    ed25519WalletPda = walletPda;
    ed25519VaultPda = vaultPda;
    ed25519OwnerAuthPda = authorityPda;
    record('CreateWallet (Ed25519 owner)', r);
  }

  let secpOwnerKey: Awaited<ReturnType<typeof generateMockSecp256r1Key>>;
  let secpWalletPda: any, secpVaultPda: any, secpOwnerAuthPda: any;
  {
    secpOwnerKey = await generateMockSecp256r1Key('lazorkit.app');
    const balBefore = await connection.getBalance(payer.publicKey);
    const { instructions, walletPda, vaultPda, authorityPda } = client.createWallet({
      payer: payer.publicKey,
      userSeed: crypto.randomBytes(32),
      owner: {
        type: 'secp256r1',
        credentialIdHash: secpOwnerKey.credentialIdHash,
        compressedPubkey: secpOwnerKey.publicKeyBytes,
        rpId: secpOwnerKey.rpId,
      },
    });
    const r = await sendAndMeasure(connection, payer, instructions);
    const balAfter = await connection.getBalance(payer.publicKey);
    r.rentCost = balBefore - balAfter - 5000;
    secpWalletPda = walletPda;
    secpVaultPda = vaultPda;
    secpOwnerAuthPda = authorityPda;
    record('CreateWallet (Secp256r1 owner)', r);
  }

  // Fund both vaults
  console.log('\n--- Fund Vaults ---');
  {
    const ix1 = SystemProgram.transfer({ fromPubkey: payer.publicKey, toPubkey: ed25519VaultPda, lamports: 0.05 * LAMPORTS_PER_SOL });
    const ix2 = SystemProgram.transfer({ fromPubkey: payer.publicKey, toPubkey: secpVaultPda, lamports: 0.05 * LAMPORTS_PER_SOL });
    const r = await sendAndMeasure(connection, payer, [ix1, ix2]);
    record('Fund 2 vaults (0.05 SOL each)', r);
  }

  // ────────────────────────────────────────────────────────────
  // 3. ADD AUTHORITY — all combinations
  // ────────────────────────────────────────────────────────────
  console.log('\n--- AddAuthority ---');

  // Ed25519 owner adds Ed25519 admin
  let ed25519AdminKp: Keypair;
  let ed25519AdminAuthPda: any;
  {
    ed25519AdminKp = Keypair.generate();
    const balBefore = await connection.getBalance(payer.publicKey);
    const { instructions, newAuthorityPda } = await client.addAuthority({
      payer: payer.publicKey,
      walletPda: ed25519WalletPda,
      adminSigner: ed25519(ed25519OwnerKp.publicKey, ed25519OwnerAuthPda),
      newAuthority: { type: 'ed25519', publicKey: ed25519AdminKp.publicKey },
      role: ROLE_ADMIN,
    });
    const r = await sendAndMeasure(connection, payer, instructions, [ed25519OwnerKp]);
    const balAfter = await connection.getBalance(payer.publicKey);
    r.rentCost = balBefore - balAfter - 5000;
    ed25519AdminAuthPda = newAuthorityPda;
    record('AddAuthority (Ed25519 owner -> Ed25519 admin)', r);
  }

  // Ed25519 admin adds Ed25519 spender
  let ed25519SpenderKp: Keypair;
  let ed25519SpenderAuthPda: any;
  {
    ed25519SpenderKp = Keypair.generate();
    const balBefore = await connection.getBalance(payer.publicKey);
    const { instructions, newAuthorityPda } = await client.addAuthority({
      payer: payer.publicKey,
      walletPda: ed25519WalletPda,
      adminSigner: ed25519(ed25519AdminKp.publicKey, ed25519AdminAuthPda),
      newAuthority: { type: 'ed25519', publicKey: ed25519SpenderKp.publicKey },
      role: ROLE_SPENDER,
    });
    const r = await sendAndMeasure(connection, payer, instructions, [ed25519AdminKp]);
    const balAfter = await connection.getBalance(payer.publicKey);
    r.rentCost = balBefore - balAfter - 5000;
    ed25519SpenderAuthPda = newAuthorityPda;
    record('AddAuthority (Ed25519 admin -> Ed25519 spender)', r);
  }

  // Secp256r1 owner adds Ed25519 admin
  let secpAdminKp: Keypair;
  let secpAdminAuthPda: any;
  {
    secpAdminKp = Keypair.generate();
    const ownerSigner = createMockSigner(secpOwnerKey);
    const balBefore = await connection.getBalance(payer.publicKey);
    const { instructions, newAuthorityPda } = await client.addAuthority({
      payer: payer.publicKey,
      walletPda: secpWalletPda,
      adminSigner: secp256r1(ownerSigner, { authorityPda: secpOwnerAuthPda }),
      newAuthority: { type: 'ed25519', publicKey: secpAdminKp.publicKey },
      role: ROLE_ADMIN,
    });
    const r = await sendAndMeasure(connection, payer, instructions);
    const balAfter = await connection.getBalance(payer.publicKey);
    r.rentCost = balBefore - balAfter - 5000;
    secpAdminAuthPda = newAuthorityPda;
    record('AddAuthority (Secp256r1 owner -> Ed25519 admin)', r);
  }

  // Secp256r1 owner adds Secp256r1 spender
  let secpSpenderKey: Awaited<ReturnType<typeof generateMockSecp256r1Key>>;
  let secpSpenderAuthPda: any;
  {
    secpSpenderKey = await generateMockSecp256r1Key('lazorkit.app');
    const ownerSigner = createMockSigner(secpOwnerKey);
    const balBefore = await connection.getBalance(payer.publicKey);
    const { instructions, newAuthorityPda } = await client.addAuthority({
      payer: payer.publicKey,
      walletPda: secpWalletPda,
      adminSigner: secp256r1(ownerSigner, { authorityPda: secpOwnerAuthPda }),
      newAuthority: {
        type: 'secp256r1',
        credentialIdHash: secpSpenderKey.credentialIdHash,
        compressedPubkey: secpSpenderKey.publicKeyBytes,
        rpId: secpSpenderKey.rpId,
      },
      role: ROLE_SPENDER,
    });
    const r = await sendAndMeasure(connection, payer, instructions);
    const balAfter = await connection.getBalance(payer.publicKey);
    r.rentCost = balBefore - balAfter - 5000;
    secpSpenderAuthPda = newAuthorityPda;
    record('AddAuthority (Secp256r1 owner -> Secp256r1 spender)', r);
  }

  // ────────────────────────────────────────────────────────────
  // 4. EXECUTE — all signer types
  // ────────────────────────────────────────────────────────────
  console.log('\n--- Execute (SOL Transfer) ---');

  // Ed25519 owner execute
  {
    const recipient = Keypair.generate().publicKey;
    const { instructions } = await client.transferSol({
      payer: payer.publicKey, walletPda: ed25519WalletPda,
      signer: ed25519(ed25519OwnerKp.publicKey, ed25519OwnerAuthPda),
      recipient, lamports: 1_000_000,
    });
    const r = await sendAndMeasure(connection, payer, instructions, [ed25519OwnerKp]);
    record('Execute SOL transfer (Ed25519 owner)', r);
  }

  // Ed25519 spender execute
  {
    const recipient = Keypair.generate().publicKey;
    const { instructions } = await client.transferSol({
      payer: payer.publicKey, walletPda: ed25519WalletPda,
      signer: ed25519(ed25519SpenderKp.publicKey, ed25519SpenderAuthPda),
      recipient, lamports: 1_000_000,
    });
    const r = await sendAndMeasure(connection, payer, instructions, [ed25519SpenderKp]);
    record('Execute SOL transfer (Ed25519 spender)', r);
  }

  // Secp256r1 owner execute
  {
    const recipient = Keypair.generate().publicKey;
    const ownerSigner = createMockSigner(secpOwnerKey);
    const { instructions } = await client.transferSol({
      payer: payer.publicKey, walletPda: secpWalletPda,
      signer: secp256r1(ownerSigner, { authorityPda: secpOwnerAuthPda }),
      recipient, lamports: 1_000_000,
    });
    const r = await sendAndMeasure(connection, payer, instructions);
    record('Execute SOL transfer (Secp256r1 owner)', r);
  }

  // Secp256r1 spender execute
  {
    const recipient = Keypair.generate().publicKey;
    const spenderSigner = createMockSigner(secpSpenderKey);
    const { instructions } = await client.transferSol({
      payer: payer.publicKey, walletPda: secpWalletPda,
      signer: secp256r1(spenderSigner, { authorityPda: secpSpenderAuthPda }),
      recipient, lamports: 1_000_000,
    });
    const r = await sendAndMeasure(connection, payer, instructions);
    record('Execute SOL transfer (Secp256r1 spender)', r);
  }

  // ────────────────────────────────────────────────────────────
  // 5. CREATE SESSION — Ed25519 and Secp256r1 admins
  // ────────────────────────────────────────────────────────────
  console.log('\n--- CreateSession ---');

  let ed25519SessionKp: Keypair;
  let ed25519SessionPda: any;
  {
    ed25519SessionKp = Keypair.generate();
    const currentSlot = BigInt(await connection.getSlot());
    const balBefore = await connection.getBalance(payer.publicKey);
    const { instructions, sessionPda } = await client.createSession({
      payer: payer.publicKey, walletPda: ed25519WalletPda,
      adminSigner: ed25519(ed25519OwnerKp.publicKey, ed25519OwnerAuthPda),
      sessionKey: ed25519SessionKp.publicKey,
      expiresAt: currentSlot + 9000n,
    });
    const r = await sendAndMeasure(connection, payer, instructions, [ed25519OwnerKp]);
    const balAfter = await connection.getBalance(payer.publicKey);
    r.rentCost = balBefore - balAfter - 5000;
    ed25519SessionPda = sessionPda;
    record('CreateSession (Ed25519 admin)', r);
  }

  let secpSessionKp: Keypair;
  let secpSessionPda: any;
  {
    secpSessionKp = Keypair.generate();
    const currentSlot = BigInt(await connection.getSlot());
    const ownerSigner = createMockSigner(secpOwnerKey);
    const balBefore = await connection.getBalance(payer.publicKey);
    const { instructions, sessionPda } = await client.createSession({
      payer: payer.publicKey, walletPda: secpWalletPda,
      adminSigner: secp256r1(ownerSigner, { authorityPda: secpOwnerAuthPda }),
      sessionKey: secpSessionKp.publicKey,
      expiresAt: currentSlot + 9000n,
    });
    const r = await sendAndMeasure(connection, payer, instructions);
    const balAfter = await connection.getBalance(payer.publicKey);
    r.rentCost = balBefore - balAfter - 5000;
    secpSessionPda = sessionPda;
    record('CreateSession (Secp256r1 admin)', r);
  }

  // ────────────────────────────────────────────────────────────
  // 6. EXECUTE VIA SESSION
  // ────────────────────────────────────────────────────────────
  console.log('\n--- Execute via Session ---');

  {
    const recipient = Keypair.generate().publicKey;
    const { instructions } = await client.transferSol({
      payer: payer.publicKey, walletPda: ed25519WalletPda,
      signer: session(ed25519SessionPda, ed25519SessionKp.publicKey),
      recipient, lamports: 1_000_000,
    });
    const r = await sendAndMeasure(connection, payer, instructions, [ed25519SessionKp]);
    record('Execute SOL transfer (Session key, Ed25519 wallet)', r);
  }

  {
    const recipient = Keypair.generate().publicKey;
    const { instructions } = await client.transferSol({
      payer: payer.publicKey, walletPda: secpWalletPda,
      signer: session(secpSessionPda, secpSessionKp.publicKey),
      recipient, lamports: 1_000_000,
    });
    const r = await sendAndMeasure(connection, payer, instructions, [secpSessionKp]);
    record('Execute SOL transfer (Session key, Secp256r1 wallet)', r);
  }

  // ────────────────────────────────────────────────────────────
  // 7. DEFERRED EXECUTION (Authorize + ExecuteDeferred)
  // ────────────────────────────────────────────────────────────
  console.log('\n--- Deferred Execution ---');

  {
    const recipient = Keypair.generate().publicKey;
    const ownerSigner = createMockSigner(secpOwnerKey);

    // TX1: Authorize
    const balBefore1 = await connection.getBalance(payer.publicKey);
    const { instructions: authIxs, deferredPayload } = await client.authorize({
      payer: payer.publicKey, walletPda: secpWalletPda,
      signer: secp256r1(ownerSigner, { authorityPda: secpOwnerAuthPda }),
      instructions: [
        SystemProgram.transfer({ fromPubkey: secpVaultPda, toPubkey: recipient, lamports: 1_000_000 }),
      ],
      expiryOffset: 300,
    });
    const r1 = await sendAndMeasure(connection, payer, authIxs);
    const balAfter1 = await connection.getBalance(payer.publicKey);
    r1.rentCost = balBefore1 - balAfter1 - 5000;
    record('Authorize (Deferred TX1, Secp256r1)', r1);

    // TX2: ExecuteDeferred
    const { instructions: execIxs } = client.executeDeferredFromPayload({
      payer: payer.publicKey,
      deferredPayload,
    });
    const r2 = await sendAndMeasure(connection, payer, execIxs);
    record('ExecuteDeferred (Deferred TX2)', r2);
  }

  // ────────────────────────────────────────────────────────────
  // 8. REMOVE AUTHORITY
  // ────────────────────────────────────────────────────────────
  console.log('\n--- RemoveAuthority ---');

  // Ed25519 owner removes Ed25519 spender
  {
    const balBefore = await connection.getBalance(payer.publicKey);
    const { instructions } = await client.removeAuthority({
      payer: payer.publicKey, walletPda: ed25519WalletPda,
      adminSigner: ed25519(ed25519OwnerKp.publicKey, ed25519OwnerAuthPda),
      targetAuthorityPda: ed25519SpenderAuthPda,
    });
    const r = await sendAndMeasure(connection, payer, instructions, [ed25519OwnerKp]);
    const balAfter = await connection.getBalance(payer.publicKey);
    r.rentCost = balBefore - balAfter - 5000; // negative = rent refund
    record('RemoveAuthority (Ed25519 owner -> Ed25519 spender)', r);
  }

  // Secp256r1 owner removes Secp256r1 spender
  {
    const ownerSigner = createMockSigner(secpOwnerKey);
    const balBefore = await connection.getBalance(payer.publicKey);
    const { instructions } = await client.removeAuthority({
      payer: payer.publicKey, walletPda: secpWalletPda,
      adminSigner: secp256r1(ownerSigner, { authorityPda: secpOwnerAuthPda }),
      targetAuthorityPda: secpSpenderAuthPda,
    });
    const r = await sendAndMeasure(connection, payer, instructions);
    const balAfter = await connection.getBalance(payer.publicKey);
    r.rentCost = balBefore - balAfter - 5000;
    record('RemoveAuthority (Secp256r1 owner -> Secp256r1 spender)', r);
  }

  // ────────────────────────────────────────────────────────────
  // 9. TRANSFER OWNERSHIP
  // ────────────────────────────────────────────────────────────
  console.log('\n--- TransferOwnership ---');

  // Ed25519 owner -> new Ed25519 owner
  {
    const newOwnerKp = Keypair.generate();
    const balBefore = await connection.getBalance(payer.publicKey);
    const { instructions, newOwnerAuthorityPda } = await client.transferOwnership({
      payer: payer.publicKey, walletPda: ed25519WalletPda,
      ownerSigner: ed25519(ed25519OwnerKp.publicKey, ed25519OwnerAuthPda),
      newOwner: { type: 'ed25519', publicKey: newOwnerKp.publicKey },
    });
    const r = await sendAndMeasure(connection, payer, instructions, [ed25519OwnerKp]);
    const balAfter = await connection.getBalance(payer.publicKey);
    r.rentCost = balBefore - balAfter - 5000;
    record('TransferOwnership (Ed25519 -> Ed25519)', r);
    // Update reference for later
    ed25519OwnerKp = newOwnerKp;
    ed25519OwnerAuthPda = newOwnerAuthorityPda;
  }

  // Secp256r1 owner -> new Secp256r1 owner
  {
    const newOwnerKey = await generateMockSecp256r1Key('lazorkit.app');
    const ownerSigner = createMockSigner(secpOwnerKey);
    const balBefore = await connection.getBalance(payer.publicKey);
    const { instructions, newOwnerAuthorityPda } = await client.transferOwnership({
      payer: payer.publicKey, walletPda: secpWalletPda,
      ownerSigner: secp256r1(ownerSigner, { authorityPda: secpOwnerAuthPda }),
      newOwner: {
        type: 'secp256r1',
        credentialIdHash: newOwnerKey.credentialIdHash,
        compressedPubkey: newOwnerKey.publicKeyBytes,
        rpId: newOwnerKey.rpId,
      },
    });
    const r = await sendAndMeasure(connection, payer, instructions);
    const balAfter = await connection.getBalance(payer.publicKey);
    r.rentCost = balBefore - balAfter - 5000;
    record('TransferOwnership (Secp256r1 -> Secp256r1)', r);
  }

  // ────────────────────────────────────────────────────────────
  // SUMMARY TABLE
  // ────────────────────────────────────────────────────────────
  console.log('\n\n' + '=' .repeat(100));
  console.log('  SUMMARY');
  console.log('=' .repeat(100));
  console.log(
    '  ' + 'Operation'.padEnd(54) +
    'CU'.padStart(8) +
    'TX Size'.padStart(10) +
    'Rent Cost'.padStart(16),
  );
  console.log('-'.repeat(100));

  for (const { label, result } of results) {
    const rentStr = result.rentCost !== undefined
      ? (result.rentCost >= 0
        ? `${(result.rentCost / LAMPORTS_PER_SOL).toFixed(6)}`
        : `${(result.rentCost / LAMPORTS_PER_SOL).toFixed(6)} (refund)`)
      : '-';
    console.log(
      '  ' + label.padEnd(54) +
      result.cu.toLocaleString().padStart(8) +
      `${result.txSize} B`.padStart(10) +
      `${rentStr} SOL`.padStart(16),
    );
  }

  console.log('=' .repeat(100));
  const finalBalance = await connection.getBalance(payer.publicKey);
  const totalSpent = payerBalance - finalBalance;
  console.log(`\nTotal spent: ${(totalSpent / LAMPORTS_PER_SOL).toFixed(6)} SOL`);
  console.log(`Final balance: ${(finalBalance / LAMPORTS_PER_SOL).toFixed(4)} SOL`);

  console.log('\nAll operations completed successfully!');
}

main().catch((err) => {
  console.error('\nFailed:', err);
  process.exit(1);
});
