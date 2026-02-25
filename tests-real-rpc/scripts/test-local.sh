#!/bin/bash
set -e

# Configuration
TEST_DIR="$(pwd)/tests-real-rpc"
SOLANA_DIR="$TEST_DIR/.test-ledger"
PROGRAM_DIR="$(cd ../program && pwd)"
DEPLOY_DIR="$(cd ../target/deploy && pwd)"

# Define cleanup function to safely shut down validator on exit
function cleanup {
    echo "-> Cleaning up..."
    if [ -n "$VALIDATOR_PID" ]; then
        kill $VALIDATOR_PID || true
    fi
    rm -rf "$SOLANA_DIR"
}
trap cleanup EXIT

echo "========================================================="
echo "🔬 Starting LazorKit Local Validator and E2E Tests..."
echo "========================================================="

# 1. Start solana-test-validator in the background
echo "-> Starting solana-test-validator..."
mkdir -p "$SOLANA_DIR"

solana-test-validator \
    --ledger "$SOLANA_DIR" \
    --bpf-program 2m47smrvCRpuqAyX2dLqPxpAC1658n1BAQga1wRCsQiT "$DEPLOY_DIR/lazorkit_program.so" \
    --reset \
    --quiet &

VALIDATOR_PID=$!

# Wait for validator to be ready
echo "-> Waiting for validator to start..."
while ! curl -s http://127.0.0.1:8899 > /dev/null; do
    sleep 1
done
echo "-> Validator is up!"

# Set connection pointing to our local node
export RPC_URL="http://127.0.0.1:8899"
export WS_URL="ws://127.0.0.1:8900"

# 2. Run Test Suite
echo "-> Running Vitest suite sequentially..."
cd "$TEST_DIR"
npm run test -- --fileParallelism=false --testTimeout=30000 --hookTimeout=30000

echo "✅ All tests completed!"
