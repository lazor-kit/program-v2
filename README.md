# LazorKit Smart Wallet (V2)

A high-performance smart wallet program on Solana with passkey (WebAuthn/Secp256r1) authentication, role-based access control, session keys, and replay-safe odometer counters. Built with [pinocchio](https://github.com/febo/pinocchio) for zero-copy serialization.

**Program ID**: `FLb7fyAtkfA4TSa2uYcAT8QKHd2pkoMHgmqfnXFXo7ao`

---

## Key Features

- **Multi-Protocol Authentication**: Ed25519 (native Solana) + Secp256r1 (WebAuthn/Passkeys/Apple Secure Enclave)
- **Role-Based Access Control**: Owner / Admin / Spender with strict permission hierarchy
- **Ephemeral Session Keys**: Time-bound keys with absolute slot-based expiry (max 30 days), revocable by Owner/Admin
- **Odometer Replay Protection**: Monotonic u32 counter per authority — works reliably with synced passkeys (iCloud, Google)
- **Clock-Based Slot Freshness**: 150-slot window via `Clock::get()` — no SlotHashes sysvar needed
- **Zero-Copy Serialization**: Raw byte casting via pinocchio, no Borsh overhead
- **CompactInstructions**: Index-based instruction packing for multi-call payloads within Solana's 1,232-byte tx limit
- **Deferred Execution**: 2-transaction flow for payloads exceeding the tx limit (e.g., Jupiter swaps) -- TX1 authorizes via signature, TX2 executes with full inner instruction space (~1,100 bytes)
- **Parallel Execution**: Different authorities on the same wallet execute concurrently -- per-authority PDA means no shared write locks
- **CPI Reentrancy Protection**: stack_height check prevents cross-program authentication attacks

---

## LazorKit vs Normal SOL Transfer

| Metric | Normal Transfer | LazorKit (Secp256r1) | LazorKit (Ed25519) | LazorKit (Session) |
|---|---|---|---|---|
| Compute Units | 150 | 9,441 | 5,864 | 4,483-5,983 |
| Transaction Size | 215 bytes | 658 bytes | 452 bytes | 452 bytes |
| Accounts | 2 | 7 | 7 | 7 |
| Instructions | 1 | 2 | 1 | 1 |
| Transaction Fee | 0.000005 SOL | 0.000005 SOL | 0.000005 SOL | 0.000005 SOL |

Session keys are ideal for frequent transactions -- they skip the Secp256r1 precompile and use a simple Ed25519 signer, resulting in lower CU and smaller transactions. All CU measurements are from real devnet transactions.

### Deferred Execution (Large Payloads)

For operations exceeding the ~574 bytes available in a single Secp256r1 Execute tx (e.g., Jupiter swaps):

| Metric | Immediate Execute | Deferred (2 txs) |
|---|---|---|
| Total CU | 9,441 | 15,613 (10,209 + 5,404) |
| Inner Ix Capacity | ~574 bytes | ~1,100 bytes (1.9x) |
| Tx Fee | 0.000005 SOL | 0.00001 SOL |
| Temp Rent | -- | 0.00212 SOL (refunded) |

See [docs/Costs.md](docs/Costs.md) for full cost analysis, session key costs, and CU benchmarks for all instructions.

---

## Cost Overview

### Rent-Exempt Costs

| Account | Data Size | Rent (SOL) |
|---|---|---|
| Wallet PDA | 8 bytes | 0.000947 |
| Authority (Ed25519) | 80 bytes | 0.001448 |
| Authority (Secp256r1) | ~125 bytes | 0.001761 |
| Session | 80 bytes | 0.001448 |
| DeferredExec | 176 bytes | 0.002116 (temporary, refunded) |

### Total Wallet Creation

| Auth Type | Total Cost | ~USD at $150/SOL |
|---|---|---|
| Ed25519 | 0.002399 SOL | $0.36 |
| Secp256r1 (Passkey) | 0.002713 SOL | $0.41 |

### Session Key Cost

| Item | Cost |
|---|---|
| Session setup (one-time rent) | 0.001453 SOL |
| Execute via session (per tx) | 0.000005 SOL |

Session rent is refundable after expiry. Ongoing Execute transactions cost only the base fee (0.000005 SOL).

---

## Architecture

| Account | Seeds | Description |
|---|---|---|
| Wallet PDA | `["wallet", user_seed]` | Identity anchor (8 bytes) |
| Vault PDA | `["vault", wallet]` | Holds SOL/tokens, program signs via PDA |
| Authority PDA | `["authority", wallet, id_hash]` | Per-key auth with role + counter |
| Session PDA | `["session", wallet, session_key]` | Ephemeral sub-key with expiry |
| DeferredExec PDA | `["deferred", wallet, authority, counter]` | Temporary pre-authorized execution (176 bytes) |

See [docs/Architecture.md](docs/Architecture.md) for struct definitions, security mechanisms, and instruction reference.

---

## Project Structure

```
program/src/           Rust smart contract (pinocchio, zero-copy)
  auth/                Ed25519 + Secp256r1/WebAuthn authentication
  processor/           9 instruction handlers
  state/               Account data structures (NoPadding)
tests-sdk/             Integration tests (vitest, 56 tests, uses @lazorkit/sdk-legacy)
docs/                  Architecture, cost analysis
audits/                Audit reports
```

---

## Quick Start

### Build

```bash
cargo build-sbf
```

### Install SDK

```bash
npm install @lazorkit/sdk-legacy
```

### Create a Wallet

```typescript
import { Connection } from '@solana/web3.js';
import { LazorKitClient } from '@lazorkit/sdk-legacy';
import * as crypto from 'crypto';

const connection = new Connection('https://api.devnet.solana.com');
const client = new LazorKitClient(connection);

const { instructions, walletPda, vaultPda } = client.createWallet({
  payer: payer.publicKey,
  userSeed: crypto.randomBytes(32),
  owner: {
    type: 'secp256r1',
    credentialIdHash,       // 32-byte SHA256 of WebAuthn credential ID
    compressedPubkey,       // 33-byte compressed Secp256r1 public key
    rpId: 'your-app.com',
  },
});
```

### Transfer SOL

```typescript
import { secp256r1 } from '@lazorkit/sdk-legacy';

// Just payer, wallet, signer, recipient, amount -- nothing else
const { instructions } = await client.transferSol({
  payer: payer.publicKey,
  walletPda,
  signer: secp256r1(mySigner),  // or ed25519(kp.publicKey) or session(sessionPda, sessionKp.publicKey)
  recipient,
  lamports: 1_000_000n,
});
```

### Execute Arbitrary Instructions

```typescript
const [vault] = client.findVault(walletPda);
const { instructions } = await client.execute({
  payer: payer.publicKey,
  walletPda,
  signer: secp256r1(mySigner),
  instructions: [
    SystemProgram.transfer({ fromPubkey: vault, toPubkey: recipient, lamports: 500_000 }),
  ],
});
```

See the [@lazorkit/sdk-legacy README](https://github.com/lazor-kit/lazorkit-protocol/tree/main/sdk/sdk-legacy) for full API reference. The same SDK transparently handles both this build (program-v2, no fees) and the commercial build (lazorkit-protocol, with fees) — it probes ProtocolConfig on first use and conditionally appends fee accounts.

---

## Testing

```bash
# Start local validator with program loaded
cd tests-sdk && npm run validator:start

# Run all 56 tests (integration + security + permission + session)
npm test

# Run CU benchmarks
npm run benchmark
```

Tests cover: wallet lifecycle, authority management, execute, deferred execution, sessions, replay protection, counter edge cases, end-to-end workflows, permission boundaries, session-based execution, and security attack vectors (reentrancy, cross-wallet isolation, accounts hash binding).

See [DEVELOPMENT.md](DEVELOPMENT.md) for full development workflow.

---

## Security

LazorKit V2 has been audited by **Accretion**.

**Status**: 17/17 security issues resolved

Security features:
- Odometer counter replay protection (per-authority monotonic u32)
- Clock-based slot freshness window (150 slots via `Clock::get()`)
- CPI reentrancy prevention (stack_height check)
- Signature binding (payer, accounts hash, counter, program_id)
- Self-removal and owner removal protection
- Session expiry validation (future + 30-day max)
- rpId stored on-chain (prevents cross-origin attacks)

Report vulnerabilities via [SECURITY.md](SECURITY.md).

---

## Documentation

| Document | Description |
|---|---|
| [Architecture](docs/Architecture.md) | Account structures, security mechanisms, instruction reference |
| [Costs](docs/Costs.md) | CU benchmarks, rent costs, transaction size analysis |
| [@lazorkit/sdk-legacy](https://github.com/lazor-kit/lazorkit-protocol/tree/main/sdk/sdk-legacy) | TypeScript SDK reference |
| [Development](DEVELOPMENT.md) | Build, test, deploy workflow |
| [Contributing](CONTRIBUTING.md) | How to contribute |
| [Security](SECURITY.md) | Vulnerability reporting |
| [Changelog](CHANGELOG.md) | Version history |

---

## License

[MIT](LICENSE)
