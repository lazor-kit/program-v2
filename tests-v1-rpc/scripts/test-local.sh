#!/bin/bash
set -e

# Configuration
TEST_DIR="$(pwd)"
SOLANA_DIR="$TEST_DIR/.test-ledger"
PROGRAM_DIR="$(cd ../program && pwd)"
DEPLOY_DIR="$(cd ../target/deploy && pwd)"

# Resolve the current deployed program id from the keypair
PROGRAM_ID=$(solana address -k "$DEPLOY_DIR/lazorkit_program-keypair.json")
echo "Resolved program id: $PROGRAM_ID"

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

# 0. Cleanup any existing validator to avoid port conflicts
echo "-> Cleaning up any existing solana-test-validator..."
if [ -f "$SOLANA_DIR/validator.pid" ]; then
    OLD_PID=$(cat "$SOLANA_DIR/validator.pid")
    kill "$OLD_PID" 2>/dev/null || true
fi
pkill -f solana-test-validator 2>/dev/null || true

# 1. Start solana-test-validator in the background
echo "-> Starting solana-test-validator..."
mkdir -p "$SOLANA_DIR"

solana-test-validator \
    --ledger "$SOLANA_DIR" \
    --bpf-program "$PROGRAM_ID" "$DEPLOY_DIR/lazorkit_program.so" \
    --reset \
    --quiet &

VALIDATOR_PID=$!
echo $VALIDATOR_PID > "$SOLANA_DIR/validator.pid"

# Wait for validator to be ready
echo "-> Waiting for validator to start..."
if ! kill -0 "$VALIDATOR_PID" 2>/dev/null; then
    echo "❌ solana-test-validator failed to start (pid exited early)."
    exit 1
fi
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

# Allow passing a specific test file or directory as argument
TEST_TARGET=${1:-"tests/"}

npm run test -- "$TEST_TARGET" --fileParallelism=false --testTimeout=30000 --hookTimeout=30000

echo "✅ All tests completed!"
