# @lazorkit/solita-client

TypeScript SDK for the LazorKit smart wallet program on Solana. Built with `@solana/web3.js` v1 and Solita-generated instruction builders.

## Installation

```bash
npm install @lazorkit/solita-client
```

## Quick Start

```typescript
import { Connection, Keypair, Transaction, sendAndConfirmTransaction } from '@solana/web3.js';
import { LazorKitClient, ed25519, secp256r1, session, ROLE_ADMIN } from '@lazorkit/solita-client';
import * as crypto from 'crypto';

const connection = new Connection('https://api.devnet.solana.com', 'confirmed');
const client = new LazorKitClient(connection);

// Create a wallet with Ed25519 owner
const owner = Keypair.generate();
const { instructions, walletPda, vaultPda, authorityPda } = client.createWallet({
  payer: payer.publicKey,
  userSeed: crypto.randomBytes(32),
  owner: { type: 'ed25519', publicKey: owner.publicKey },
});
await sendAndConfirmTransaction(connection, new Transaction().add(...instructions), [payer]);

// Or with Secp256r1 (passkey) owner
const { instructions: ixs2 } = client.createWallet({
  payer: payer.publicKey,
  userSeed: crypto.randomBytes(32),
  owner: {
    type: 'secp256r1',
    credentialIdHash,      // 32-byte SHA256 of WebAuthn credential ID
    compressedPubkey,      // 33-byte compressed public key
    rpId: 'your-app.com',
  },
});
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

### Signer Types (Discriminated Unions)

The SDK uses discriminated union types for signers. Helper constructors make creation concise:

```typescript
import { ed25519, secp256r1, session } from '@lazorkit/solita-client';

// Ed25519 — Keypair signs at transaction level
const signer = ed25519(ownerKp.publicKey, authorityPda);  // authorityPda optional (auto-derived)

// Secp256r1 — passkey/WebAuthn
const signer = secp256r1(myPasskeySigner, { authorityPda, slotOverride });  // both optional

// Session — ephemeral key
const signer = session(sessionPda, sessionKp.publicKey);
```

**Type unions:**
- `AdminSigner` = `Ed25519SignerConfig | Secp256r1SignerConfig` -- for admin operations (addAuthority, removeAuthority, transferOwnership, createSession)
- `ExecuteSigner` = above + `SessionSignerConfig` -- for execute/transferSol

### High-Level Client API

Every method returns `{ instructions: TransactionInstruction[]; ...extraPdas }`. The client auto-derives PDAs, auto-fetches slots, auto-reads counters, auto-packs compact instructions, and auto-computes accounts hashes.

```typescript
import { LazorKitClient, ed25519, secp256r1, session, ROLE_ADMIN, ROLE_SPENDER } from '@lazorkit/solita-client';

const client = new LazorKitClient(connection);

// ── Read counter ──
const counter = await client.readCounter(authorityPda);

// ── Create wallet (unified for both auth types) ──
const { instructions, walletPda, vaultPda, authorityPda } = client.createWallet({
  payer: payer.publicKey,
  userSeed,
  owner: { type: 'ed25519', publicKey: ownerKp.publicKey },
  // or: owner: { type: 'secp256r1', credentialIdHash, compressedPubkey, rpId: 'app.com' },
});

// ── Add authority (unified — works with any admin signer) ──
const { instructions, newAuthorityPda } = await client.addAuthority({
  payer: payer.publicKey,
  walletPda,
  adminSigner: ed25519(ownerKp.publicKey, ownerAuthPda),  // or secp256r1(signer)
  newAuthority: { type: 'ed25519', publicKey: adminKp.publicKey },
  role: ROLE_ADMIN,
});

// ── Remove authority ──
const { instructions } = await client.removeAuthority({
  payer: payer.publicKey,
  walletPda,
  adminSigner: ed25519(adminKp.publicKey, adminAuthPda),
  targetAuthorityPda: spenderAuthPda,
  // refundDestination defaults to payer
});

// ── Transfer ownership ──
const { instructions, newOwnerAuthorityPda } = await client.transferOwnership({
  payer: payer.publicKey,
  walletPda,
  ownerSigner: secp256r1(ceoSigner),
  newOwner: { type: 'secp256r1', credentialIdHash, compressedPubkey, rpId },
});

// ── Execute (unified — Ed25519, Secp256r1, or Session) ──
const [vault] = client.findVault(walletPda);
const { instructions } = await client.execute({
  payer: payer.publicKey,
  walletPda,
  signer: secp256r1(mySigner),  // or ed25519(kp.publicKey) or session(sessionPda, sessionKp.publicKey)
  instructions: [
    SystemProgram.transfer({ fromPubkey: vault, toPubkey: recipient, lamports: 1_000_000 }),
  ],
});
// Note: for Ed25519 add ownerKp to tx signers, for Session add sessionKp

// ── Transfer SOL (convenience) ──
const { instructions } = await client.transferSol({
  payer: payer.publicKey,
  walletPda,
  signer: secp256r1(mySigner),
  recipient,
  lamports: 1_000_000n,
});

// ── Create session ──
const { instructions, sessionPda } = await client.createSession({
  payer: payer.publicKey,
  walletPda,
  adminSigner: ed25519(ownerKp.publicKey, ownerAuthPda),
  sessionKey: sessionKp.publicKey,
  expiresAt: currentSlot + 9000n,
});

// ── Deferred Execution — TX1 (Authorize) ──
const { instructions, deferredExecPda, deferredPayload } = await client.authorize({
  payer: payer.publicKey,
  walletPda,
  signer: secp256r1(mySigner),  // Secp256r1 only
  instructions: [jupiterSwapIx],
  expiryOffset: 300, // ~2 minutes in slots
});

// ── Deferred Execution — TX2 (ExecuteDeferred) ──
const { instructions: tx2Ixs } = client.executeDeferredFromPayload({
  payer: payer.publicKey,
  deferredPayload,  // returned from authorize()
  // refundDestination defaults to payer
});

// ── Reclaim expired DeferredExec (refund rent) ──
const { instructions } = client.reclaimDeferred({
  payer: payer.publicKey,
  deferredExecPda,
  // refundDestination defaults to payer
});
```

All Secp256r1 signers accept an optional `slotOverride` for batching scenarios.

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
