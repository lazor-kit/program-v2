---
description: how to run local integration and logic tests for LazorKit
---

This workflow ensures all tests (Rust logic and TypeScript E2E) are executed correctly using the unified script.

1. **Prerequisites**: Ensure you have Solana and Rust tools installed.
// turbo
2. **Execute Unified Test Suite**:
   Run the root-level script. This handles build, validator setup, and TS tests.
   ```bash
   ./scripts/test.sh
   ```

3. **Execute Rust-Only Logic Tests**:
   For faster feedback on program-level logic changes:
   ```bash
   cd program && cargo test-sbf
   ```

4. **Debugging**:
   If a test fails, logs are redirected to stdout. If the validator doesn't clean up, use:
   ```bash
   pkill -f solana-test-validator
   ```
