
# LazorKit Solita TypeScript SDK

> Modern, robust TypeScript SDK for the LazorKit smart wallet protocol on Solana. Extreme detail: usage for every exported API, account mapping, batching, advanced cryptography, test/dev flows, and full RBAC. Everything below is tested and production-ready.

---

## :package: Installation

```bash
npm install @lazorkit/solita-client
# or

```

---

## :rocket: Instant Bootstrapping Example

```ts
import { LazorClient, AuthType, Role } from "@lazorkit/solita-client";
import { Keypair, Connection } from "@solana/web3.js";

const connection = new Connection("https://api.devnet.solana.com");
const payer = Keypair.generate();
const owner = Keypair.generate();
const client = new LazorClient(connection);

// --- 1. Create Ed25519 wallet ---
const { ix, walletPda, authorityPda, userSeed } = await client.createWallet({
  payer,
  authType: AuthType.Ed25519,
  owner: owner.publicKey,
});
// ...compose/send transaction

// --- 2. Add Admin Authority ---
const newAdmin = Keypair.generate();
await client.addAuthority({
  payer,
  walletPda,
  newAuthPubkey: newAdmin.publicKey.toBytes(),
  newAuthType: AuthType.Ed25519,
  role: Role.Admin,
  adminType: AuthType.Ed25519,
  adminSigner: owner,
});
```

---

## :page_facing_up: SDK API – Full Usage for All Functions

### LazorClient (high-level API)

> See also [tests-v1-rpc/tests/](../../tests-v1-rpc/tests/) for real flows with all functions in action.
All methods return objects with instructions (ix) and all derived addresses.

#### `createWallet(params)`

Create a new wallet with either Ed25519 or Passkey (Secp256r1) owner.

```ts
// Ed25519 owner
await client.createWallet({ payer, authType: AuthType.Ed25519, owner: pubkey });

// Passkey/Secp256r1 owner
await client.createWallet({ payer, authType: AuthType.Secp256r1, pubkey, credentialHash });
```

**Returns:** `{ ix, walletPda, authorityPda, userSeed }`

#### `addAuthority(params)` / `removeAuthority(params)` / `changeRole(params)`
Add/remove/upgrade admin, owner, spender. Both Ed25519 and Passkey. Requires admin/owner signature or credential.

```ts
await client.addAuthority({ payer, walletPda, newAuthPubkey, role: Role.Admin, adminType: AuthType.Ed25519, adminSigner });
```

#### `createSession(params)` / `closeSession(params)`
Programmatic sessions/child key, slot-tied expiry, reclaim rent after closing.
```ts
await client.createSession({ payer, walletPda, authorityPda, sessionKey, expiresAt });
await client.closeSession({ payer, walletPda, sessionPda, configPda, adminSigner });
```

#### `execute(params)`
Run any instructions (single or compact/batched, CU-efficient, full RBAC/slot checking under the hood).
```ts
await client.execute({ payer, walletPda, authorityPda, innerInstructions: [ /* . . . */ ], signer });
```

#### PDA & Derivation Helpers

```ts
client.getWalletPda(userSeed)
client.getVaultPda(walletPda)
client.getAuthorityPda(walletPda, idSeed)
client.getSessionPda(walletPda, sessionKey)
client.getConfigPda()
client.getTreasuryShardPda(shardId)
```

#### Accessing raw instruction builders
If you need lowest level control, use `client.builder`: All contract instructions prepared for direct assembly/deep composition.

---

### find* PDAs - account derivation

All functions take (seed,[programId?]). Returns `[PublicKey, bump]` arrays for direct PDA math.

```ts
findWalletPda(userSeed)
findVaultPda(walletPubkey)
findAuthorityPda(walletPubkey,idSeed)
findSessionPda(walletPubkey,sessionKey)
findConfigPda()
findTreasuryShardPda(shardId)
```

---

### Compact Packing (Batching)

Fully supported for highest-throughput; contract expects this layout in Execute.

#### `packCompactInstructions(instructions: CompactInstruction[])`

```ts
import { packCompactInstructions } from "@lazorkit/solita-client";
// instructions: array of {programIdIndex, accountIndexes, data}
const packed = packCompactInstructions([ ... ]);
```

#### `computeAccountsHash(accountMetas, instructions)`

Strict matching with on-chain. Supply full AccountMeta[] as per your actual call.
```ts
const hash = await computeAccountsHash(accountMetas, instructions);
```

---

### Secp256r1 & WebAuthn Tools

- `Secp256r1Signer` interface: implement for custom signers (browser WebAuthn, hardware)
- `appendSecp256r1Sysvars(ix)` – auto-injects correct sysvar accounts for secp/slot
- `buildAuthPayload(...)` – builds proof-of-liveness WebAuthn payload
- `readCurrentSlot(connection)` – load latest slot for nonce checks

---

### Account Layout Types & Constants

- `AUTHORITY_ACCOUNT_HEADER_SIZE`, `AUTHORITY_ACCOUNT_ED25519_SIZE`, `AUTHORITY_ACCOUNT_SECP256R1_SIZE` - memory mapping helpers.
- `Role` and `AuthType` enums.

---

## :triangular_flag_on_post: Reference: Real-world End-to-End

- [tests-v1-rpc/tests/02-wallet.test.ts](../../tests-v1-rpc/tests/02-wallet.test.ts) (main flows)
- [03-authority.test.ts](../../tests-v1-rpc/tests/03-authority.test.ts) (admin/owner mgmt)
- [04-execute.test.ts](../../tests-v1-rpc/tests/04-execute.test.ts) (multi-tx/cross-auth)

---

## Security/Dev Notes

- All calls strictly respect contract's RBAC logic and zero-copy struct boundaries
- Passkey/web crypto flows require correct signature prep; see helper/test for padding/hash details
- Packing code and address layout is 1:1 with the Rust contract; you can even print struct offsets for debugging

---

## License
MIT

---

## :construction: Core API (LazorClient)

### Wallet & Authority
- `createWallet(params)` — Deploys all 3: Wallet, Vault, initial Owner authority. Ed25519 or Secp256r1 (Passkey) supported. Returns PDAs + actual seed.
- `addAuthority(params)` — (Owner/Admin only) Add additional roles: Owner, Admin, Spender. Ed25519 and Passkey supported.
- `removeAuthority(params)` — Remove an authority PDA (role drop/delegation)
- `changeRole(params)` — Upgrade/downgrade roles for a given PDA

### Sessions / Temp Keys
- `createSession(params)` — Spawn a session sub-key, set expiry. (Authority may be Ed25519/Secp256r1)
- `closeSession(params)` — Closes session, returns rent to payer (authorized via admin or expiry)

### Execution
- `execute(params)` — Core instruction executor. Accepts a batch (compact packed or regular) and authority/session parameters. All role/slot/auth checks enforced by program.

### Advanced/Batch
- Use `packCompactInstructions` and related utilities for batch/multi-call flows (see `packing.ts`)

### PDA Helpers (Everywhere)
- `findWalletPda(userSeed)`
- `findAuthorityPda(walletPda, idSeed)`
- `findSessionPda(walletPda, sessionKey)`
- `findVaultPda(walletPda)`
- `findConfigPda()`
- `findTreasuryShardPda(shardId)`

---

## :key: Account & Memory Model (On-chain Mapping)

All account layouts map **exactly** to Rust structs (see [../program/src/state/](../../program/src/state)): zero-copy, no Borsh/Serde. See [Architecture.md](../../docs/Architecture.md) for diagrams.

Quick summary:

- **WalletAccount**: { discriminator, bump, version, _padding[5] }
- **AuthorityAccountHeader**: { discriminator, authority_type, role, bump, version, _padding[3], counter, wallet }
- **SessionAccount**: { discriminator, bump, version, _padding[5], wallet, session_key, expires_at }
- **ConfigAccount**: { discriminator, bump, version, num_shards, _padding[4], admin, wallet_fee, action_fee }

*Do not attempt to read/write account memory using other layouts—always use the auto-generated classes in `generated/` (e.g., `AuthorityAccount.fromAccountAddress(connection, authorityPda)`).*

---

## :chart_with_upwards_trend: Example: WebAuthn/Passkey Authority

```ts
// Generate secp256r1 authority (browser or Node.js; see test Suite)
// 1. Generate credential ID, get 32-byte hash
const credentialIdHash = sha256(...); // depends on your WebAuthn lib
const compressedPubKey = ...; // Uint8Array, 33 bytes, from crypto API

const { ix, walletPda, authorityPda } = await client.createWallet({
  payer,
  authType: AuthType.Secp256r1,
  pubkey: compressedPubKey,
  credentialHash: credentialIdHash,
});
```

For E2E usage, see [tests-v1-rpc/tests/03-authority.test.ts](../../tests-v1-rpc/tests/03-authority.test.ts) and [src/utils/secp256r1.ts](src/utils/secp256r1.ts) for P-256 flows, including precompile instruction helpers, payload encoding, and more.

---

## :hammer_and_wrench: Advanced: Batching & Compaction

- For maximum CU efficiency, batch multiple instructions using `packCompactInstructions()` before `.execute()`
- See [04-execute.test.ts](../../tests-v1-rpc/tests/04-execute.test.ts) for real examples with SOL/SPL transfers, and full invocation of compacted instructions.

---

## :rocket: Testing & Development

- E2E test suite: [tests-v1-rpc/tests/](../../tests-v1-rpc/tests/)
- Contract core: [../program/src/](../../program/src/)
- Architecture, diagrams: [../docs/Architecture.md](../../docs/Architecture.md)

---

## :triangular_flag_on_post: Security & Gotchas

- All account memory must use correct layout/encoding (NoPadding), or on-chain checks will fail
- Pay attention to role/authority order — only Authority PDAs tied to the correct role can invoke privileged flows
- Session expiry is **absolute** slot-based (epoch/slot math may differ DevNet/MainNet)
- secp256r1 (Passkey) precompile flows require extra sysvar and signature preload; see helper and test examples
- Shard selection for fees is done via payer pubkey mod numShards (uses direct bytes)

---

## :link: Resources

- [Architecture.md](../../docs/Architecture.md)
- [Contract logic reference (program/src/)](../../program/src/)
- [Tests / E2E integration](../../tests-v1-rpc/tests/)
- [Solana web3.js](https://solana-labs.github.io/solana-web3.js/)

---

## License
MIT
