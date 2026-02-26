# LazorKit Development Workflow

This document outlines the standard procedures for building, deploying, and testing the LazorKit program and its associated SDK.

## 1. Prerequisites
- [Solana Tool Suite](https://docs.solanalabs.com/cli/install) (latest stable)
- [Rust](https://www.rust-lang.org/tools/install)
- [Node.js & npm](https://nodejs.org/)
- [Shank CLI](https://github.com/metaplex-foundation/shank) (for IDL generation)

## 2. Project Structure
- `/program`: Rust smart contract (Pinocchio-based)
- `/sdk/lazorkit-ts`: TypeScript SDK generated via Codama
- `/tests-real-rpc`: Integration tests running against Devnet
- `/scripts`: Automation utility scripts

## 3. Core Workflows

### A. Program ID Synchronization
Whenever you redeploy the program to a new address, run the sync script to update all references (Rust, SDK generator, and tests):
```bash
./scripts/sync-program-id.sh <YOUR_NEW_PROGRAM_ID>
```
*This command automatically regenerates the SDK.*

### B. IDL & SDK Generation
If you change the instructions or account structures in Rust, you must update the IDL and then the SDK:
1. **Update IDL** (using Shank):
   ```bash
   cd program && shank idl -o . --out-filename idl.json -p <YOUR_PROGRAM_ID>
   ```
2. **Regenerate SDK**:
   ```bash
   cd sdk/lazorkit-ts && npm run generate
   ```

### C. Testing on Devnet
Tests are optimized for Devnet with rate-limiting protection (exponential backoff and sequential execution).

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
1. **Deploy Program**:
   ```bash
   solana program deploy program/target/deploy/lazorkit_program.so -u d
   ```
2. **Publish IDL to Blockchain** (So explorers can show your contract functions):
   ```bash
   # Run from root directory
   npx --force @solana-program/program-metadata write idl <YOUR_PROGRAM_ID> ./program/idl.json
   ```

## 4. Troubleshooting
- **429 Too Many Requests**: The test suite handles this automatically with a retry loop. If failures persist, check your RPC provider credits or increase the sleep delay in `tests/common.ts`.
- **Simulation Failed (Already Initialized)**: Devnet accounts persist. Change the `userSeed` in your test file or use a fresh `getRandomSeed()` to create new wallet instances.
- **BigInt Serialization Error**: Always use the provided `tryProcessInstruction` helper in `common.ts` for catching errors, as it handles BigInt conversion for logging.
