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
- [Node.js & npm](https://nodejs.org/)
- [Shank CLI](https://github.com/metaplex-foundation/shank) (for IDL generation)
- [Codama CLI](https://github.com/metaplex-foundation/codama) (for SDK generation)

## 2. Project Structure
- `/program`: Rust smart contract (Pinocchio-based)
  - Highly optimized, zero-copy architecture (`NoPadding`).
- `/sdk/lazorkit-ts`: TypeScript SDK generated via Codama.
  - Contains generated instructions for interaction with the contract.
- `/tests-real-rpc`: Integration tests running against a live RPC (Devnet/Localhost).
- `/scripts`: Automation utility scripts (e.g., syncing program IDs).

## 3. Core Workflows

### A. Program ID Synchronization
Whenever you redeploy the program to a new address, run the sync script to update all references across Rust, the SDK generator, and your tests:
```bash
./scripts/sync-program-id.sh <YOUR_NEW_PROGRAM_ID>
```
*Note: This script will update hardcoded IDs and typically trigger SDK regeneration automatically.*

### B. IDL & SDK Generation
If you modify instruction parameters or account structures in the Rust program, you must regenerate both the IDL and the SDK:
1. **Update IDL** (using Shank):
   ```bash
   cd program && shank idl -o . --out-filename idl.json -p <YOUR_PROGRAM_ID>
   ```
2. **Regenerate SDK** (using Codama):
   ```bash
   cd sdk/lazorkit-ts && npm run generate
   ```

### C. Testing & Validation
Tests are built to run against an actual RPC node (`tests-real-rpc`), ensuring realistic validation of behaviors like `SlotHashes` nonce verification and resource limits.

1. **Setup Env**: Ensure `.env` in `tests-real-rpc/` has your `PRIVATE_KEY`, `RPC_URL`, and `WS_URL`.
2. **Run All Tests**:
   ```bash
   cd tests-real-rpc && npm run test:devnet
   ```
3. **Run Single Test File** (Recommended for debugging):
   ```bash
   cd tests-real-rpc && npm run test:devnet:file tests/instructions/create_wallet.test.ts
   ```

### D. Deployment & IDL Publishing
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
