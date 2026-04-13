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
import { findWalletPda, findVaultPda, findAuthorityPda, findSessionPda } from '@lazorkit/solita-client';

// Derive wallet PDA from user seed
const [walletPda, walletBump] = findWalletPda(userSeed);

// Derive vault PDA from wallet
const [vaultPda, vaultBump] = findVaultPda(walletPda);

// Derive authority PDA from wallet + credential hash (or pubkey for Ed25519)
const [authorityPda, authBump] = findAuthorityPda(walletPda, credentialIdHash);

// Derive session PDA from wallet + session key
const [sessionPda, sessionBump] = findSessionPda(walletPda, sessionKeyBytes);
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

### Compact Instruction Packing

Pack multiple instructions for Execute:

```typescript
import { packCompactInstructions, computeAccountsHash } from '@lazorkit/solita-client';

// Define compact instructions with account indexes (not pubkeys)
const packed = packCompactInstructions([{
  programIdIndex: 6,        // Index of SystemProgram in accounts
  accountIndexes: [3, 7],   // vault (from), recipient (to)
  data: transferData,
}]);

// For Secp256r1: compute accounts hash for signature binding
const accountsHash = computeAccountsHash(accountMetas, compactInstructions);
```

### Secp256r1 (Passkey) Utilities

```typescript
import {
  readAuthorityCounter,
  buildAuthPayload,
  buildSecp256r1Challenge,
  type Secp256r1Signer,
} from '@lazorkit/solita-client';

// Read current counter from on-chain authority account
const counter = await readAuthorityCounter(connection, authorityPda);

// Build auth payload for Secp256r1 signing
const authPayload = buildAuthPayload({
  slot: BigInt(currentSlot),
  counter: counter + 1n,
  sysvarIxIndex: 4,
  sysvarSlotHashesIndex: 5,
  typeAndFlags: 0x10,          // webauthn.get + https
  rpId: 'lazorkit.app',
  authenticatorData: authData,
});

// Build challenge hash (7 elements)
const challenge = buildSecp256r1Challenge({
  discriminator: new Uint8Array([4]),  // Execute
  authPayload,
  signedPayload,
  slot: BigInt(currentSlot),
  payer: payerPublicKey,
  counter: counter + 1n,
});
```

#### Secp256r1Signer Interface

```typescript
interface Secp256r1Signer {
  publicKeyBytes: Uint8Array;      // 33-byte compressed pubkey
  credentialIdHash: Uint8Array;    // 32-byte SHA256 of credential ID
  rpId: string;                    // e.g., "lazorkit.app"
  sign(challenge: Uint8Array): Promise<{
    signature: Uint8Array;         // 64-byte raw signature
    authenticatorData: Uint8Array; // WebAuthn authenticator data
  }>;
}
```

### High-Level Client

`LazorKitClient` provides convenience methods:

```typescript
import { LazorKitClient } from '@lazorkit/solita-client';

const client = new LazorKitClient(connection);

// Read counter
const counter = await client.readCounter(authorityPda);

// Create wallet (Ed25519)
const { ix, walletPda, vaultPda, authorityPda } = client.createWalletEd25519({
  payer, userSeed, ownerPubkey,
});

// Create wallet (Secp256r1)
const { ix, walletPda, vaultPda, authorityPda } = client.createWalletSecp256r1({
  payer, userSeed, credentialIdHash, compressedPubkey,
});

// Execute (Ed25519)
const ix = client.executeEd25519({
  payer, walletPda, authorityPda, vaultPda, compactInstructions, remainingAccounts,
});

// Execute (Secp256r1) — includes precompile instruction
const { ix, precompileIx } = await client.executeSecp256r1({
  payer, walletPda, authorityPda, vaultPda,
  signer, slot, sysvarIxIndex, sysvarSlotHashesIndex,
  compactInstructions, remainingAccounts,
});
```

### Constants

```typescript
// Instruction discriminators
DISC_CREATE_WALLET    // 0
DISC_ADD_AUTHORITY    // 1
DISC_REMOVE_AUTHORITY // 2
DISC_TRANSFER_OWNERSHIP // 3
DISC_EXECUTE          // 4
DISC_CREATE_SESSION   // 5

// Auth types
AUTH_TYPE_ED25519     // 0
AUTH_TYPE_SECP256R1   // 1

// Roles
ROLE_OWNER   // 0
ROLE_ADMIN   // 1
ROLE_SPENDER // 2

// Program ID
PROGRAM_ID   // 2m47smrvCRpuqAyX2dLqPxpAC1658n1BAQga1wRCsQiT
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
| 3007 | InvalidSignatureAge | Slot outside SlotHashes window |
| 3008 | InvalidSessionDuration | Session expiry out of range |
| 3009 | SessionExpired | Session past expires_at slot |
| 3010 | AuthorityDoesNotSupportSession | N/A |
| 3011 | InvalidAuthenticationKind | Unknown authority_type |
| 3012 | InvalidMessage | N/A |
| 3013 | SelfReentrancyNotAllowed | CPI back into program rejected |

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
cd program && shank idl -o . --out-filename idl.json -p 2m47smrvCRpuqAyX2dLqPxpAC1658n1BAQga1wRCsQiT

# 2. Regenerate SDK
cd sdk/solita-client && node generate.mjs
```

## License

MIT
