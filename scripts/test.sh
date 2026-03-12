#!/bin/bash
set -e

# LazorKit Unified Test Script
# This script builds the program and runs integration tests against a local validator.

ROOT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
PROGRAM_DIR="$ROOT_DIR/program"
TARGET_DIR="$ROOT_DIR/target/deploy"
TEST_DIR="$ROOT_DIR/tests-real-rpc"
LEDGER_DIR="$TEST_DIR/.test-ledger"

# 1. Build and Run Rust Logic Tests
echo "🔨 Building LazorKit and running Rust tests..."
cd "$PROGRAM_DIR"
cargo test-sbf
cd "$ROOT_DIR"

# 2. Get Program ID (default or from keypair)
PROGRAM_ID=$(solana address -k "$TARGET_DIR/lazorkit_program-keypair.json")
echo "📍 Program ID: $PROGRAM_ID"

# 3. Cleanup existing validator
cleanup() {
    echo "🧹 Cleaning up..."
    if [ -f "$LEDGER_DIR/validator.pid" ]; then
        PID=$(cat "$LEDGER_DIR/validator.pid")
        kill $PID 2>/dev/null || true
    fi
    pkill -f solana-test-validator 2>/dev/null || true
    # rm -rf "$LEDGER_DIR" # Keeps ledger for debugging if needed, or remove if fresh start preferred
}
trap cleanup EXIT

# 4. Start local validator
echo "🚀 Starting solana-test-validator..."
mkdir -p "$LEDGER_DIR"
solana-test-validator \
    --ledger "$LEDGER_DIR" \
    --bpf-program "$PROGRAM_ID" "$TARGET_DIR/lazorkit_program.so" \
    --reset \
    --quiet &
VALIDATOR_PID=$!
echo $VALIDATOR_PID > "$LEDGER_DIR/validator.pid"

# Wait for validator
echo "⏳ Waiting for validator to be ready..."
while ! curl -s http://127.0.0.1:8899 > /dev/null; do
    sleep 1
done
echo "✅ Validator is up!"

# 5. Run tests
echo "🧪 Running TypeScript integration tests..."
export RPC_URL="http://127.0.0.1:8899"
export WS_URL="ws://127.0.0.1:8900"
cd "$TEST_DIR"
npm run test -- --fileParallelism=false --testTimeout=30000

echo "🎉 All tests passed!"
