#!/bin/bash
set -e

# Configuration
TEST_DIR="$(pwd)/tests-real-rpc"
SOLANA_DIR="$TEST_DIR/.test-ledger"
PROGRAM_DIR="$(cd ../program && pwd)"
DEPLOY_DIR="$(cd ../target/deploy && pwd)"

echo "========================================================="
echo "🔬 Starting LazorKit Local Validator and E2E Tests..."
echo "========================================================="

# 1. Start solana-test-validator in the background
echo "-> Starting solana-test-validator..."
mkdir -p "$SOLANA_DIR"

solana-test-validator \
    --ledger "$SOLANA_DIR" \
    --bpf-program Btg4mLUdMd3ov8PBtmuuFMAimLAdXyew9XmsGtuY9VcP "$DEPLOY_DIR/lazorkit_program.so" \
    --reset \
    --quiet &

VALIDATOR_PID=$!

# Wait for validator to be ready
echo "-> Waiting for validator to start (5 seconds)..."
sleep 5

# Set connection pointing to our local node
export RPC_URL="http://127.0.0.1:8899"

# 2. Run Test Suite
echo "-> Running Vitest suite..."
cd "$TEST_DIR"
npm run test

# 3. Clean up
echo "-> Cleaning up..."
kill $VALIDATOR_PID
rm -rf "$SOLANA_DIR"

echo "✅ All tests completed!"
