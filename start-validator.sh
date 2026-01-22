#!/bin/bash
set -e

PROGRAM_ID=$(solana address -k target/deploy/lazorkit_program-keypair.json)

echo "🚀 Starting Solana Test Validator..."
echo "Program ID: $PROGRAM_ID"

solana-test-validator \
  --bpf-program $PROGRAM_ID target/deploy/lazorkit_program.so \
  --reset \
  --quiet &

VALIDATOR_PID=$!
echo "Validator PID: $VALIDATOR_PID"

# Wait for validator
sleep 5

if ! solana cluster-version --url localhost &>/dev/null; then
    echo "❌ Validator failed to start"
    exit 1
fi

echo "✅ Validator running on http://localhost:8899"
echo ""
echo "To stop: kill $VALIDATOR_PID"
echo "To view logs: solana logs --url localhost"
