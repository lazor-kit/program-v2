# LazorKit Development Workflow

This document outlines the standard procedures for building, deploying, and testing the LazorKit program and its associated SDK.

## Prerequisites

- [Solana Tool Suite](https://docs.solanalabs.com/cli/install) (v2.x+)
- [Rust](https://www.rust-lang.org/tools/install) (via rustup)
- [Node.js 18+](https://nodejs.org/) & npm
- [shank-cli](https://github.com/metaplex-foundation/shank): `cargo install shank-cli`

## Project Structure

```
/program           Rust smart contract (pinocchio, zero-copy)
/tests-sdk          Integration tests (vitest, @lazorkit/sdk-legacy)
/scripts            Build/deploy automation
/audits             Audit reports
/no-padding         Custom NoPadding derive macro
/assertions         Custom assertion helpers

The TypeScript SDK lives in the sibling `lazorkit-protocol` repo at
`sdk/sdk-legacy/` and is published to npm as `@lazorkit/sdk-legacy`. The
same SDK transparently handles both this build (program-v2, no fees) and
the commercial build — it probes ProtocolConfig on first use and
conditionally appends fee accounts.
```

## Core Workflows

### A. Build Program

The program ID is chosen at build time via the `mainnet` / `devnet` cargo features
(see `assertions/src/lib.rs`). Exactly one must be set; an unflagged build fails
with a `compile_error!`.

```bash
# Devnet build — embeds FLb7fyAtkfA4TSa2uYcAT8QKHd2pkoMHgmqfnXFXo7ao
cargo build-sbf --features devnet

# Mainnet build — embeds LazorjRFNavitUaBu5m3WaNPjU1maipvSW2rZfAFAKi
# (slot is shared with lazorkit-protocol — see "Mainnet Deploy Strategy" below)
cargo build-sbf --features mainnet
```

The convenience script `./scripts/build-all.sh <devnet|mainnet>` builds and
regenerates the IDL in one shot. SDK regeneration is no longer needed —
`@lazorkit/sdk-legacy` is hand-written and lives in the sibling repo.

### B. Run Rust Tests

```bash
cargo test --features devnet
```

The `--features devnet` flag is required because the assertions crate's
`compile_error!` fires on un-flagged builds. Choose either feature — host-side
tests use a runtime `program_id: Pubkey::new_unique()`, so the embedded ID
doesn't affect test outcomes.

### C. IDL Generation (using Shank)

```bash
cd program
PROGRAM_ID=$(solana-keygen pubkey ../target/deploy/lazorkit_program-keypair.json)
shank idl -o . --out-filename idl.json -p "$PROGRAM_ID"
```

### D. SDK

`@lazorkit/sdk-legacy` is hand-written (no codegen) and lives in the
sibling `lazorkit-protocol` repo at `sdk/sdk-legacy/`. To use it locally:

```bash
# In a sibling checkout: /Users/.../lazorkit-protocol/sdk/sdk-legacy
npm install && npm run build

# Then in this repo's tests-sdk (already configured via `file:` link):
cd tests-sdk && npm install
```

Once published to npm, consumers do `npm install @lazorkit/sdk-legacy`.

### E. Running Integration Tests

```bash
# Terminal 1: Start local validator with program loaded
cd tests-sdk && npm run validator:start

# Terminal 2: Run all 56 tests
cd tests-sdk && npm test
```

### F. Running Benchmarks

```bash
cd tests-sdk && npm run benchmark
```

Measures CU usage and transaction sizes for all instructions, including deferred execution (Authorize TX1 + ExecuteDeferred TX2).

### G. Deploy to Devnet

```bash
cargo build-sbf --features devnet
solana program deploy target/deploy/lazorkit_program.so -u d
```

### H. Mainnet Deploy Strategy (Foundation Build)

program-v2 is the no-fee "foundation" variant of LazorKit. For the duration of
the foundation contract, its mainnet binary occupies the SAME mainnet program
slot as `lazorkit-protocol`'s commercial binary — `LazorjRFNavitUaBu5m3WaNPjU1maipvSW2rZfAFAKi`.
dApp integrators keep using one stable program ID; only the on-chain behavior
changes when the binary is swapped.

**During contract:**

```bash
cargo build-sbf --features mainnet
solana program deploy target/deploy/lazorkit_program.so -u m
# Optionally pin the upgrade authority to a multisig held jointly by foundation
# and the lazorkit team so the post-contract swap can happen.
```

**At contract end** — swap to lazorkit-protocol's commercial binary:

```bash
# In the lazorkit-protocol repo:
cargo build-sbf --features mainnet
solana program deploy target/deploy/lazorkit_program.so -u m \
  --program-id LazorjRFNavitUaBu5m3WaNPjU1maipvSW2rZfAFAKi
```

The upgrade authority key must control the `LazorjRF…` slot for both deploys.
There is no separate program-v2 vanity keypair to manage.

## Troubleshooting

- **429 Too Many Requests**: Check RPC credits or use local validator.
- **Already Initialized**: Use fresh userSeed or reset validator with `--reset`.
- **InvalidSeeds**: Verify PDA derivation matches on-chain seeds.
- **0xbc0 (InvalidSessionDuration)**: expires_at must be a future slot, not Unix timestamp.
