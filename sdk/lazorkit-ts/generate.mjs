/**
 * LazorKit SDK Code Generation Script
 * 
 * Converts the Shank IDL to a Codama root node, enriches it with
 * account types, error codes, PDA definitions, and enum types,
 * then renders a TypeScript client.
 * 
 * Usage: node generate.mjs
 */
import { readFileSync } from 'fs';
import { join, dirname } from 'path';
import { fileURLToPath } from 'url';
import { rootNodeFromAnchor } from '@codama/nodes-from-anchor';
import { renderVisitor } from '@codama/renderers-js';
import { createFromRoot, visit } from 'codama';

const __dirname = dirname(fileURLToPath(import.meta.url));

// ─── 1. Read Shank IDL ───────────────────────────────────────────
const idlPath = join(__dirname, '../../program/idl.json');
const idl = JSON.parse(readFileSync(idlPath, 'utf-8'));
console.log('✓ Read IDL from', idlPath);

// ─── 2. Inject program address (missing from Shank IDL) ─────────
idl.metadata = idl.metadata || {};
idl.metadata.address = 'Btg4mLUdMd3ov8PBtmuuFMAimLAdXyew9XmsGtuY9VcP';
console.log('✓ Injected program address');

// ─── 2b. Patch account metadata ─────────────────────────────────
// adminAuthority is a PDA (cannot sign directly). The actual Ed25519
// signer is authorizerSigner. Shank marks adminAuthority as isSigner 
// because the contract reads it, but Codama interprets this as 
// "must provide a TransactionSigner". Fix it.
for (const ix of idl.instructions) {
    for (const acc of ix.accounts) {
        // adminAuthority → PDA, not a signer
        if (acc.name === 'adminAuthority') {
            acc.isSigner = false;
        }
        // payer should be writable (pays rent)
        if (acc.name === 'payer') {
            acc.isMut = true;
        }
    }

    // Patch instruction arguments to match runtime layouts (C-structs)
    if (ix.name === 'CreateWallet') {
        // Runtime: [user_seed(32), auth_type(1), auth_bump(1), padding(6), ...payload]
        // IDL originally: [userSeed, authType, authPubkey, credentialHash]

        // 1. Fix userSeed type from 'bytes' (variable) to '[u8; 32]' (fixed)
        const userSeed = ix.args.find(a => a.name === 'userSeed');
        if (userSeed) userSeed.type = { array: ['u8', 32] };

        // 2. Inject missing fields & replace payload
        const authTypeIdx = ix.args.findIndex(a => a.name === 'authType');
        if (authTypeIdx !== -1) {
            ix.args.splice(authTypeIdx + 1, 0,
                { name: 'authBump', type: 'u8' },
                { name: 'padding', type: { array: ['u8', 6] } }
            );

            // Remove old auth args (authPubkey, credentialHash) and add generic payload
            // Finding them by name removes assumption of index
            const argsToRemove = ['authPubkey', 'credentialHash'];
            ix.args = ix.args.filter(a => !argsToRemove.includes(a.name));
            ix.args.push({ name: 'payload', type: 'bytes' });
        }
    }

    if (ix.name === 'AddAuthority') {
        // Fix adminAuthority signer status (it's a PDA, verified via payload, not tx signer)
        const adminAuth = ix.accounts.find(a => a.name === 'adminAuthority');
        if (adminAuth) adminAuth.isSigner = false;

        // Runtime: [authority_type(1), new_role(1), padding(6), ...payload]
        // IDL originally: [newType, newPubkey, newHash, newRole]

        const newRoleIdx = ix.args.findIndex(a => a.name === 'newRole');
        const newRoleArg = ix.args[newRoleIdx];

        // Remove newRole from end
        ix.args.splice(newRoleIdx, 1);

        // Insert newRole + padding after newType
        const newTypeIdx = ix.args.findIndex(a => a.name === 'newType');
        ix.args.splice(newTypeIdx + 1, 0,
            newRoleArg,
            { name: 'padding', type: { array: ['u8', 6] } }
        );

        // Replace payload args
        const argsToRemove = ['newPubkey', 'newHash'];
        ix.args = ix.args.filter(a => !argsToRemove.includes(a.name));
        ix.args.push({ name: 'payload', type: 'bytes' });
    }

    if (ix.name === 'RemoveAuthority') {
        const adminAuth = ix.accounts.find(a => a.name === 'adminAuthority');
        if (adminAuth) adminAuth.isSigner = false;
    }

    if (ix.name === 'CreateSession') {
        const adminAuth = ix.accounts.find(a => a.name === 'adminAuthority');
        if (adminAuth) adminAuth.isSigner = false;
    }

    if (ix.name === 'TransferOwnership') {
        const currOwner = ix.accounts.find(a => a.name === 'currentOwnerAuthority');
        if (currOwner) currOwner.isSigner = false;

        // Helper to replace payload args for TransferOwnership too?
        // TransferOwnershipArgs in runtime: [auth_type(1)] followed by payload
        // IDL: [newType, newPubkey, newHash]
        // We should replace newPubkey, newHash with payload
        const argsToRemove = ['newPubkey', 'newHash'];
        ix.args = ix.args.filter(a => !argsToRemove.includes(a.name));
        ix.args.push({ name: 'payload', type: 'bytes' });
    }
}
console.log('✓ Patched account metadata & instruction layouts');

// ─── 3. Add account types ────────────────────────────────────────
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
console.log('✓ Added 3 account types');

// ─── 4. Add error codes ──────────────────────────────────────────
idl.errors = [
    { code: 3001, name: 'InvalidAuthorityPayload', msg: 'Invalid authority payload' },
    { code: 3002, name: 'PermissionDenied', msg: 'Permission denied' },
    { code: 3003, name: 'InvalidInstruction', msg: 'Invalid instruction' },
    { code: 3004, name: 'InvalidPubkey', msg: 'Invalid public key' },
    { code: 3005, name: 'InvalidMessageHash', msg: 'Invalid message hash' },
    { code: 3006, name: 'SignatureReused', msg: 'Signature has already been used' },
    { code: 3007, name: 'InvalidSignatureAge', msg: 'Invalid signature age' },
    { code: 3008, name: 'InvalidSessionDuration', msg: 'Invalid session duration' },
    { code: 3009, name: 'SessionExpired', msg: 'Session has expired' },
    { code: 3010, name: 'AuthorityDoesNotSupportSession', msg: 'Authority type does not support sessions' },
    { code: 3011, name: 'InvalidAuthenticationKind', msg: 'Invalid authentication kind' },
    { code: 3012, name: 'InvalidMessage', msg: 'Invalid message' },
    { code: 3013, name: 'SelfReentrancyNotAllowed', msg: 'Self-reentrancy is not allowed' },
];
console.log('✓ Added 13 error codes');

// ─── 5. Add enum types ───────────────────────────────────────────
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
                { name: 'Wallet' },
                { name: 'Authority' },
                { name: 'Session' },
            ],
        },
    },
);
console.log('✓ Added 3 enum types');

// ─── 6. Convert to Codama root node ──────────────────────────────
const rootNode = rootNodeFromAnchor(idl);
console.log('✓ Converted enriched IDL to Codama root node');

// ─── 7. Create Codama instance ───────────────────────────────────
const codama = createFromRoot(rootNode);
console.log('✓ Created Codama instance');

// ─── 8. Render to TypeScript ─────────────────────────────────────
const outputDir = join(__dirname, 'src', 'generated');
console.log('  Rendering to', outputDir);

visit(codama.getRoot(), renderVisitor(outputDir));
console.log('✓ Done! Generated files in src/generated/');
