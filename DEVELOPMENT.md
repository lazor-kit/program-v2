# LazorKit Development Workflow

This document outlines the standard procedures for building, deploying, and testing the LazorKit program and its associated SDK.

## 🚀 Local Quick Start (Recommended)
If you are developing locally, you can run everything (Build + Validator + Tests) with a single command:
```bash
./scripts/test.sh
```
This script ensures a clean environment, builds the latest program, and runs the full Vitest suite against a local validator.

## 1. Prerequisites
- [Solana Tool Suite](https://docs.solanalabs.com/cli/install) (latest stable)
- [Rust](https://www.rust-lang.org/tools/install)
- [Node.js, npm & pnpm](https://nodejs.org/)
- [Solita](https://github.com/metaplex-foundation/solita) (for legacy web3.js v1 SDK generation)

## 2. Project Structure
- `/program`: Rust smart contract (Pinocchio-based)
  - Highly optimized, zero-copy architecture (`NoPadding`).
- `/sdk/solita-client`: TypeScript SDK generated via Solita & manually augmented.
  - Contains generated instructions wrapped by a high-level `LazorClient` to manage derivations and Secp256r1 webauth payloads.
- `/tests-v1-rpc`: Integration tests running against a local test validator using Vitest and legacy `@solana/web3.js` (v1).
- `/scripts`: Automation utility scripts.

## 3. Core Workflows

### A. Program ID Synchronization
Whenever you redeploy the program to a new address, ensure you update the `PROGRAM_ID` constants in:
- `program/src/lib.rs` (Declare id)
- `sdk/solita-client/src/utils/pdas.ts`

### B. SDK Generation & Augmentation
If you modify instruction parameters or account structures in the Rust program, you must regenerate the SDK:
1. **Regenerate SDK** (using Solita):
   ```bash
   cd program && yarn solita
   ```
2. **Rebuild Client Wrapper**:
   Since the smart contract uses strict `[repr(C)]` / `NoPadding` layouts, the generated `beet` serializers from Solita often inject a 4-byte padding prefix. Lay out custom parameter inputs manually within `sdk/solita-client/src/utils/client.ts` to construct precise buffer offsets.
   ```bash
   cd sdk/solita-client && pnpm run build
   ```

### C. Testing & Validation
Tests run exclusively on a localized `solana-test-validator` to guarantee execution determinism, specifically for verifying the `SlotHashes` sysvar.

1. **Run Full Test Suite** (Recommended for full validation):
   ```bash
   cd tests-v1-rpc && ./scripts/test-local.sh
   ```
   *Note: This script will spawn the validator, await fee stabilization, and trigger all 69 Vitest endpoints sequentially.*

2. **Run Single Test File**:
   ```bash
   cd tests-v1-rpc && vitest run tests/06-ownership.test.ts
   ```
1. **Build the Program**:
   ```bash
   cargo build-sbf
   ```
2. **Deploy Program**:
   ```bash
   solana program deploy target/deploy/lazorkit_program.so -u d
   ```
3. **Publish IDL to Blockchain** (So block explorers can decode your contract interactions):
   ```bash
   # Run from root directory
   npx --force @solana-program/program-metadata write idl <YOUR_PROGRAM_ID> ./program/idl.json
   ```

## 4. Troubleshooting
- **429 Too Many Requests**: The test suite handles this automatically with a retry loop. If failures persist, check your RPC provider credits or increase the sleep delay in `tests/common.ts`.
- **Simulation Failed (Already Initialized)**: Devnet accounts persist. Change the `userSeed` in your test file or use a fresh `getRandomSeed()` to create new wallet instances.
- **BigInt Serialization Error**: Always use the provided `tryProcessInstruction` helper in `common.ts` for catching errors, as it automatically handles `BigInt` conversion for logging.
- **InvalidSeeds / auth_payload errors**: Ensure your generated `auth_payload` respects the exact `Codama` layout and is correctly appended to instruction data.
