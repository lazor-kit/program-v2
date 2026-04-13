/**
 * LazorKit Solita Code Generation Script
 *
 * Reads the Shank IDL, enriches it with account types, error codes,
 * and enum types, then generates TypeScript via Solita.
 *
 * Usage: node generate.mjs
 */
import { readFileSync, writeFileSync, mkdirSync } from 'fs';
import { join, dirname } from 'path';
import { fileURLToPath } from 'url';
import { Solita } from '@metaplex-foundation/solita';

const __dirname = dirname(fileURLToPath(import.meta.url));

// ─── 1. Read Shank IDL ───────────────────────────────────────────
const idlPath = join(__dirname, '../../program/idl.json');
const idl = JSON.parse(readFileSync(idlPath, 'utf-8'));
console.log('Read IDL from', idlPath);

// ─── 2. Inject program address ──────────────────────────────────
idl.metadata = idl.metadata || {};
idl.metadata.address = 'FLb7fyAtkfA4TSa2uYcAT8QKHd2pkoMHgmqfnXFXo7ao';

// ─── 3. Add account types ───────────────────────────────────────
idl.accounts = [
  {
    name: 'WalletAccount',
    type: {
      kind: 'struct',
      fields: [
        { name: 'discriminator', type: 'u8' },
        { name: 'bump', type: 'u8' },
        { name: 'version', type: 'u8' },
        { name: 'padding', type: { array: ['u8', 5] } },
      ],
    },
  },
  {
    name: 'AuthorityAccount',
    type: {
      kind: 'struct',
      fields: [
        { name: 'discriminator', type: 'u8' },
        { name: 'authorityType', type: 'u8' },
        { name: 'role', type: 'u8' },
        { name: 'bump', type: 'u8' },
        { name: 'version', type: 'u8' },
        { name: 'padding', type: { array: ['u8', 3] } },
        { name: 'counter', type: 'u64' },
        { name: 'wallet', type: 'publicKey' },
      ],
    },
  },
  {
    name: 'SessionAccount',
    type: {
      kind: 'struct',
      fields: [
        { name: 'discriminator', type: 'u8' },
        { name: 'bump', type: 'u8' },
        { name: 'version', type: 'u8' },
        { name: 'padding', type: { array: ['u8', 5] } },
        { name: 'wallet', type: 'publicKey' },
        { name: 'sessionKey', type: 'publicKey' },
        { name: 'expiresAt', type: 'u64' },
      ],
    },
  },
];
console.log('Added 3 account types');

// ─── 4. Add error codes ─────────────────────────────────────────
idl.errors = [
  { code: 3001, name: 'InvalidAuthorityPayload', msg: 'Invalid authority payload' },
  { code: 3002, name: 'PermissionDenied', msg: 'Permission denied' },
  { code: 3003, name: 'InvalidInstruction', msg: 'Invalid instruction' },
  { code: 3004, name: 'InvalidPubkey', msg: 'Invalid public key' },
  { code: 3005, name: 'InvalidMessageHash', msg: 'Invalid message hash' },
  { code: 3006, name: 'SignatureReused', msg: 'Signature has already been used (counter mismatch)' },
  { code: 3007, name: 'InvalidSignatureAge', msg: 'Signature too old (outside 150-slot window)' },
  { code: 3008, name: 'InvalidSessionDuration', msg: 'Invalid session duration' },
  { code: 3009, name: 'SessionExpired', msg: 'Session has expired' },
  { code: 3010, name: 'AuthorityDoesNotSupportSession', msg: 'Authority type does not support sessions' },
  { code: 3011, name: 'InvalidAuthenticationKind', msg: 'Invalid authentication kind' },
  { code: 3012, name: 'InvalidMessage', msg: 'Invalid message' },
  { code: 3013, name: 'SelfReentrancyNotAllowed', msg: 'Self-reentrancy is not allowed' },
];
console.log('Added 13 error codes');

// ─── 5. Add enum types ──────────────────────────────────────────
if (!idl.types) idl.types = [];
idl.types.push(
  {
    name: 'AuthorityType',
    type: {
      kind: 'enum',
      variants: [
        { name: 'Ed25519' },
        { name: 'Secp256r1' },
      ],
    },
  },
  {
    name: 'Role',
    type: {
      kind: 'enum',
      variants: [
        { name: 'Owner' },
        { name: 'Admin' },
        { name: 'Spender' },
      ],
    },
  },
  {
    name: 'AccountDiscriminator',
    type: {
      kind: 'enum',
      variants: [
        { name: 'Uninitialized' },
        { name: 'Wallet' },
        { name: 'Authority' },
        { name: 'Session' },
      ],
    },
  },
);
console.log('Added 3 enum types');

// ─── 6. Write enriched IDL ──────────────────────────────────────
const enrichedPath = join(__dirname, 'idl-enriched.json');
writeFileSync(enrichedPath, JSON.stringify(idl, null, 2));
console.log('Wrote enriched IDL to', enrichedPath);

// ─── 7. Generate via Solita ─────────────────────────────────────
const outputDir = join(__dirname, 'src', 'generated');
mkdirSync(outputDir, { recursive: true });

const gen = new Solita(idl, { programName: 'lazorkit_program', programId: idl.metadata.address });
await gen.renderAndWriteTo(outputDir);
console.log('Generated TypeScript to', outputDir);
