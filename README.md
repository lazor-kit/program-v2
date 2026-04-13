# LazorKit Smart Wallet (V2)

A high-performance smart wallet program on Solana with passkey (WebAuthn/Secp256r1) authentication, role-based access control, session keys, and replay-safe odometer counters. Built with [pinocchio](https://github.com/febo/pinocchio) for zero-copy serialization.

**Program ID**: `FLb7fyAtkfA4TSa2uYcAT8QKHd2pkoMHgmqfnXFXo7ao`

---

## Key Features

- **Multi-Protocol Authentication**: Ed25519 (native Solana) + Secp256r1 (WebAuthn/Passkeys/Apple Secure Enclave)
- **Role-Based Access Control**: Owner / Admin / Spender with strict permission hierarchy
- **Ephemeral Session Keys**: Time-bound keys with absolute slot-based expiry (max 30 days)
- **Odometer Replay Protection**: Monotonic u32 counter per authority — works reliably with synced passkeys (iCloud, Google)
- **Clock-Based Slot Freshness**: 150-slot window via `Clock::get()` — no SlotHashes sysvar needed
- **Zero-Copy Serialization**: Raw byte casting via pinocchio, no Borsh overhead
- **CompactInstructions**: Index-based instruction packing for multi-call payloads within Solana's 1,232-byte tx limit
- **Deferred Execution**: 2-transaction flow for payloads exceeding the tx limit (e.g., Jupiter swaps) -- TX1 authorizes via signature, TX2 executes with full inner instruction space (~1,100 bytes)
- **CPI Reentrancy Protection**: stack_height check prevents cross-program authentication attacks

---

## LazorKit vs Normal SOL Transfer

| Metric | Normal Transfer | LazorKit (Secp256r1) | LazorKit (Session) |
|---|---|---|---|
| Compute Units | 150 | 12,441 | 8,983 |
| Transaction Size | 215 bytes | 658 bytes | 452 bytes |
| Accounts | 2 | 7 | 7 |
| Instructions | 1 | 2 | 1 |
| Transaction Fee | 0.000005 SOL | 0.000005 SOL | 0.000005 SOL |

Session keys are ideal for frequent transactions -- they skip the Secp256r1 precompile and use a simple Ed25519 signer, resulting in lower CU and smaller transactions.

### Deferred Execution (Large Payloads)

For operations exceeding the ~574 bytes available in a single Secp256r1 Execute tx (e.g., Jupiter swaps):

| Metric | Immediate Execute | Deferred (2 txs) |
|---|---|---|
| Total CU | 12,441 | 18,613 (11,709 + 6,904) |
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
sdk/solita-client/     TypeScript SDK (Solita-generated + hand-written utils)
  src/generated/       Auto-generated instructions, accounts, errors
  src/utils/           Instruction builders, PDA helpers, signing utils
tests-sdk/             Integration tests (vitest, 35 tests)
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
npm install @lazorkit/solita-client
```

### Create a Wallet (Ed25519)

```typescript
import { Connection, Keypair } from '@solana/web3.js';
import { LazorKitClient } from '@lazorkit/solita-client';
import * as crypto from 'crypto';

const connection = new Connection('https://api.devnet.solana.com');
const client = new LazorKitClient(connection);

const owner = Keypair.generate();
const userSeed = crypto.randomBytes(32);

const { ix, walletPda, vaultPda } = client.createWalletEd25519({
  payer: payer.publicKey,
  userSeed,
  ownerPubkey: owner.publicKey,
});
```

See [sdk/solita-client/README.md](sdk/solita-client/README.md) for full API reference.

---

## Testing

```bash
# Start local validator with program loaded
cd tests-sdk && npm run validator:start

# Run all 35 integration tests
npm test

# Run CU benchmarks
npm run benchmark
```

Tests cover: wallet lifecycle, authority management, execute, deferred execution, sessions, replay protection, counter edge cases, and end-to-end workflows.

See [DEVELOPMENT.md](DEVELOPMENT.md) for full development workflow.

---

## Security

LazorKit V2 has been audited by **Accretion** (Solana Foundation funded).

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
| [SDK API](sdk/solita-client/README.md) | TypeScript SDK reference |
| [Development](DEVELOPMENT.md) | Build, test, deploy workflow |
| [Contributing](CONTRIBUTING.md) | How to contribute |
| [Security](SECURITY.md) | Vulnerability reporting |
| [Changelog](CHANGELOG.md) | Version history |

---

## License

[MIT](LICENSE)
