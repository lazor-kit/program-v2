# @lazorkit/solita-client

TypeScript SDK for the LazorKit smart wallet program on Solana. Built with `@solana/web3.js` v1 and Solita-generated instruction builders.

## Installation

```bash
npm install @lazorkit/solita-client
```

## Quick Start

```typescript
import { Connection, Keypair, LAMPORTS_PER_SOL } from '@solana/web3.js';
import { LazorKitClient, AUTH_TYPE_ED25519 } from '@lazorkit/solita-client';
import * as crypto from 'crypto';

const connection = new Connection('https://api.devnet.solana.com', 'confirmed');
const client = new LazorKitClient(connection);

// Create a wallet with Ed25519 authentication
const payer = Keypair.generate();
const owner = Keypair.generate();
const userSeed = crypto.randomBytes(32);

const { ix, walletPda, vaultPda, authorityPda } = client.createWalletEd25519({
  payer: payer.publicKey,
  userSeed,
  ownerPubkey: owner.publicKey,
});

// Send the transaction
const tx = new Transaction().add(ix);
await sendAndConfirmTransaction(connection, tx, [payer]);
```

## API Reference

### PDA Helpers

```typescript
import { findWalletPda, findVaultPda, findAuthorityPda, findSessionPda, findDeferredExecPda } from '@lazorkit/solita-client';

// Derive wallet PDA from user seed
const [walletPda, walletBump] = findWalletPda(userSeed);

// Derive vault PDA from wallet
const [vaultPda, vaultBump] = findVaultPda(walletPda);

// Derive authority PDA from wallet + credential hash (or pubkey for Ed25519)
const [authorityPda, authBump] = findAuthorityPda(walletPda, credentialIdHash);

// Derive session PDA from wallet + session key
const [sessionPda, sessionBump] = findSessionPda(walletPda, sessionKeyBytes);

// Derive deferred execution PDA from wallet + authority + counter
const [deferredPda, deferredBump] = findDeferredExecPda(walletPda, authorityPda, counter);
```

### Instruction Builders

Low-level builders that return `TransactionInstruction`:

```typescript
import {
  createCreateWalletIx,
  createAddAuthorityIx,
  createRemoveAuthorityIx,
  createTransferOwnershipIx,
  createExecuteIx,
  createCreateSessionIx,
  createAuthorizeIx,
  createExecuteDeferredIx,
  createReclaimDeferredIx,
} from '@lazorkit/solita-client';
```

#### createCreateWalletIx

```typescript
const ix = createCreateWalletIx({
  payer: PublicKey,
  walletPda: PublicKey,
  vaultPda: PublicKey,
  authorityPda: PublicKey,
  userSeed: Uint8Array,        // 32 bytes
  authType: number,            // AUTH_TYPE_ED25519 (0) or AUTH_TYPE_SECP256R1 (1)
  authBump: number,
  credentialOrPubkey: Uint8Array, // Ed25519: 32-byte pubkey | Secp256r1: 32-byte credential_id_hash
  secp256r1Pubkey?: Uint8Array,   // Secp256r1 only: 33-byte compressed pubkey
  rpId?: string,                  // Secp256r1 only: relying party ID (e.g., "lazorkit.app")
});
```

#### createExecuteIx

```typescript
const ix = createExecuteIx({
  payer: PublicKey,
  walletPda: PublicKey,
  authorityPda: PublicKey,
  vaultPda: PublicKey,
  packedInstructions: Uint8Array,  // From packCompactInstructions()
  authPayload?: Uint8Array,        // Secp256r1 only
  remainingAccounts?: AccountMeta[],
});
```

#### createAuthorizeIx (Deferred Execution TX1)

```typescript
const ix = createAuthorizeIx({
  payer: PublicKey,
  walletPda: PublicKey,
  authorityPda: PublicKey,
  deferredExecPda: PublicKey,
  instructionsHash: Uint8Array,  // 32 bytes — SHA256 of packed compact instructions
  accountsHash: Uint8Array,      // 32 bytes — SHA256 of all referenced account pubkeys
  expiryOffset: number,          // Slots until expiry (10-9000)
  authPayload: Uint8Array,       // Secp256r1 auth payload
});
```

#### createExecuteDeferredIx (Deferred Execution TX2)

```typescript
const ix = createExecuteDeferredIx({
  payer: PublicKey,
  walletPda: PublicKey,
  vaultPda: PublicKey,
  deferredExecPda: PublicKey,
  refundDestination: PublicKey,
  packedInstructions: Uint8Array,   // From packCompactInstructions()
  remainingAccounts?: AccountMeta[], // Inner accounts referenced by instructions
});
```

#### createReclaimDeferredIx

```typescript
const ix = createReclaimDeferredIx({
  payer: PublicKey,
  deferredExecPda: PublicKey,
  refundDestination: PublicKey,
});
```

### Compact Instruction Packing

Pack multiple instructions for Execute:

```typescript
import { packCompactInstructions, computeAccountsHash, computeInstructionsHash } from '@lazorkit/solita-client';

// Define compact instructions with account indexes (not pubkeys)
const packed = packCompactInstructions([{
  programIdIndex: 5,        // Index of SystemProgram in accounts
  accountIndexes: [3, 6],   // vault (from), recipient (to)
  data: transferData,
}]);

// For Secp256r1: compute accounts hash for signature binding
const accountsHash = computeAccountsHash(accountMetas, compactInstructions);

// For deferred execution: compute instructions hash (signed in TX1, verified in TX2)
const instructionsHash = computeInstructionsHash(compactInstructions);
```

### Secp256r1 (Passkey) Utilities

```typescript
import {
  readAuthorityCounter,
  buildAuthPayload,
  buildSecp256r1Challenge,
  generateAuthenticatorData,
  type Secp256r1Signer,
} from '@lazorkit/solita-client';

// Read current counter from on-chain authority account
const counter = await readAuthorityCounter(connection, authorityPda);

// Generate WebAuthn authenticator data from RP ID (37 bytes: rpIdHash + flags + counter)
const authenticatorData = generateAuthenticatorData('lazorkit.app');

// Build auth payload for Secp256r1 signing
const authPayload = buildAuthPayload({
  slot: BigInt(currentSlot),
  counter: counter + 1,          // number (u32), not bigint
  sysvarIxIndex: 4,
  typeAndFlags: 0x10,            // webauthn.get + https
  authenticatorData: authData,   // rpId is stored on-chain, not sent per-tx
});

// Build challenge hash (7 elements)
const challenge = buildSecp256r1Challenge({
  discriminator: new Uint8Array([4]),  // Execute
  authPayload,
  signedPayload,
  slot: BigInt(currentSlot),
  payer: payerPublicKey,
  counter: counter + 1,          // number (u32)
});
```

#### Secp256r1Signer Interface

```typescript
interface Secp256r1Signer {
  publicKeyBytes: Uint8Array;      // 33-byte compressed pubkey
  credentialIdHash: Uint8Array;    // 32-byte SHA256 of credential ID
  rpId: string;                    // e.g., "lazorkit.app"
  sign(challenge: Uint8Array): Promise<{
    signature: Uint8Array;          // 64-byte raw signature (r||s), low-S normalized
    authenticatorData: Uint8Array;  // WebAuthn authenticator data
    clientDataJsonHash: Uint8Array; // SHA256 of clientDataJSON
  }>;
}
```

### High-Level Methods (Simplified DX)

The simplest way to use LazorKit -- no compact instructions, no account indexes, no remaining accounts:

```typescript
import { LazorKitClient } from '@lazorkit/solita-client';

const client = new LazorKitClient(connection);

// Transfer SOL -- just pass payer, wallet, signer, recipient, amount
const ixs = await client.transferSol({
  payer: payer.publicKey,
  walletPda,
  signer,          // Secp256r1Signer from your WebAuthn flow
  recipient,       // destination PublicKey
  lamports: 1_000_000n,
});
await sendAndConfirmTransaction(connection, new Transaction().add(...ixs), [payer]);

// Execute arbitrary instructions -- pass standard TransactionInstructions
const [vault] = client.findVault(walletPda);
const ixs = await client.execute({
  payer: payer.publicKey,
  walletPda,
  signer,
  instructions: [
    SystemProgram.transfer({ fromPubkey: vault, toPubkey: recipient, lamports: 1_000_000 }),
    // ... any other instructions
  ],
});
await sendAndConfirmTransaction(connection, new Transaction().add(...ixs), [payer]);
```

Both methods auto-derive PDAs, auto-fetch the current slot, auto-read the counter, auto-pack compact instructions, and auto-compute the accounts hash. The returned array includes the Secp256r1 precompile instruction followed by the Execute instruction.

### Full Client API

`LazorKitClient` auto-derives PDAs (vaultPda from walletPda), auto-fetches slot from connection, and auto-computes sysvar instruction indexes. Only pass what's truly required:

```typescript
import { LazorKitClient } from '@lazorkit/solita-client';

const client = new LazorKitClient(connection);

// Read counter
const counter = await client.readCounter(authorityPda);

// Create wallet (Ed25519)
const { ix, walletPda, vaultPda, authorityPda } = client.createWalletEd25519({
  payer, userSeed, ownerPubkey,
});

// Create wallet (Secp256r1) — rpId stored on-chain
const { ix, walletPda, vaultPda, authorityPda } = client.createWalletSecp256r1({
  payer, userSeed, credentialIdHash, compressedPubkey, rpId: 'lazorkit.app',
});

// Add authority (Secp256r1 admin) — slot auto-fetched, sysvarIxIndex auto-computed
const { ix, newAuthorityPda, precompileIx } = await client.addAuthoritySecp256r1({
  payer, walletPda, adminAuthorityPda, adminSigner: signer,
  newType: AUTH_TYPE_ED25519, newRole: ROLE_SPENDER,
  newCredentialOrPubkey: newPubkeyBytes,
});

// Remove authority (Secp256r1 admin)
const { ix, precompileIx } = await client.removeAuthoritySecp256r1({
  payer, walletPda, adminAuthorityPda, adminSigner: signer,
  targetAuthorityPda,
  // refundDestination defaults to payer
});

// Execute (Ed25519) — vaultPda auto-derived
const ix = client.executeEd25519({
  payer, walletPda, authorityPda, compactInstructions, remainingAccounts,
});

// Execute (Secp256r1) — vaultPda, slot, sysvarIxIndex all auto-derived
const { ix, precompileIx } = await client.executeSecp256r1({
  payer, walletPda, authorityPda, signer,
  compactInstructions, remainingAccounts,
});

// Execute (Session key) — vaultPda auto-derived, session key as signer
const ix = client.executeSession({
  payer, walletPda, sessionPda, sessionKeyPubkey,
  compactInstructions, remainingAccounts,
});
// Note: sessionKeypair must be added as tx signer: [payer, sessionKeypair]

// Create session (Secp256r1 admin)
const { ix, sessionPda, precompileIx } = await client.createSessionSecp256r1({
  payer, walletPda, adminAuthorityPda, adminSigner: signer,
  sessionKey, expiresAt,
});

// Transfer ownership (Secp256r1)
const { ix, newOwnerAuthorityPda, precompileIx } = await client.transferOwnershipSecp256r1({
  payer, walletPda, currentOwnerAuthorityPda, ownerSigner: signer,
  newType: AUTH_TYPE_ED25519, newCredentialOrPubkey,
});

// Deferred Execution — TX1 (Authorize)
const { authorizeIx, precompileIx, deferredExecPda } = await client.authorizeSecp256r1({
  payer, walletPda, authorityPda, signer,
  compactInstructions, tx2AccountMetas,
  expiryOffset: 300, // ~2 minutes
});

// Deferred Execution — TX2 (ExecuteDeferred) — vaultPda auto-derived
const ix = client.executeDeferred({
  payer, walletPda, deferredExecPda,
  compactInstructions, remainingAccounts,
  // refundDestination defaults to payer
});

// Reclaim expired DeferredExec (refund rent)
const ix = client.reclaimDeferred({
  payer, deferredExecPda,
  // refundDestination defaults to payer
});
```

All Secp256r1 methods accept an optional `slotOverride` param for batching scenarios where you want to control the slot value.

### Constants

```typescript
// Instruction discriminators
DISC_CREATE_WALLET    // 0
DISC_ADD_AUTHORITY    // 1
DISC_REMOVE_AUTHORITY // 2
DISC_TRANSFER_OWNERSHIP // 3
DISC_EXECUTE          // 4
DISC_CREATE_SESSION   // 5
DISC_AUTHORIZE        // 6
DISC_EXECUTE_DEFERRED // 7
DISC_RECLAIM_DEFERRED // 8

// Auth types
AUTH_TYPE_ED25519     // 0
AUTH_TYPE_SECP256R1   // 1

// Roles
ROLE_OWNER   // 0
ROLE_ADMIN   // 1
ROLE_SPENDER // 2

// Program ID
PROGRAM_ID   // FLb7fyAtkfA4TSa2uYcAT8QKHd2pkoMHgmqfnXFXo7ao
```

### Error Handling

```typescript
import { extractErrorCode, ERROR_NAMES } from '@lazorkit/solita-client';

try {
  await sendAndConfirmTransaction(connection, tx, [payer]);
} catch (err) {
  const code = extractErrorCode(err);
  if (code) {
    console.log(`Error: ${ERROR_NAMES[code]} (${code})`);
  }
}
```

Error codes:
| Code | Name | Description |
|------|------|-------------|
| 3001 | InvalidAuthorityPayload | Malformed auth payload |
| 3002 | PermissionDenied | Insufficient role permissions |
| 3003 | InvalidInstruction | Precompile instruction verification failed |
| 3004 | InvalidPubkey | Public key mismatch |
| 3005 | InvalidMessageHash | Challenge hash mismatch |
| 3006 | SignatureReused | Counter mismatch (replay attempt) |
| 3007 | InvalidSignatureAge | Slot too old (>150 slots from current) |
| 3008 | InvalidSessionDuration | Session expiry out of range |
| 3009 | SessionExpired | Session past expires_at slot |
| 3010 | AuthorityDoesNotSupportSession | N/A |
| 3011 | InvalidAuthenticationKind | Unknown authority_type |
| 3012 | InvalidMessage | N/A |
| 3013 | SelfReentrancyNotAllowed | CPI back into program rejected |
| 3014 | DeferredAuthorizationExpired | DeferredExec past expires_at slot |
| 3015 | DeferredHashMismatch | Instructions or accounts hash mismatch |
| 3016 | InvalidExpiryWindow | Expiry offset out of range (10-9000 slots) |
| 3017 | UnauthorizedReclaim | Only original payer can reclaim |
| 3018 | DeferredAuthorizationNotExpired | Cannot reclaim before expiry |

### Generated Accounts

Solita-generated account classes with `fromAccountAddress()`:

```typescript
import { WalletAccount, AuthorityAccount, SessionAccount } from '@lazorkit/solita-client';

const wallet = await WalletAccount.fromAccountAddress(connection, walletPda);
const authority = await AuthorityAccount.fromAccountAddress(connection, authorityPda);
const session = await SessionAccount.fromAccountAddress(connection, sessionPda);
```

## SDK Regeneration

After modifying program instructions:

```bash
# 1. Regenerate IDL
cd program && shank idl -o . --out-filename idl.json -p FLb7fyAtkfA4TSa2uYcAT8QKHd2pkoMHgmqfnXFXo7ao

# 2. Regenerate SDK
cd sdk/solita-client && node generate.mjs
```

## License

MIT
