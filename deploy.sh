#!/bin/bash
set -e

# Default variables
RPC_URL=${RPC_URL:-"https://api.devnet.solana.com"}
KEYPAIR=${KEYPAIR:-"$HOME/.config/solana/id.json"}
PROGRAM_DIR="./program"

echo "=========================================="
echo "LazorKit Deployment Script"
echo "RPC URL: $RPC_URL"
echo "Keypair: $KEYPAIR"
echo "=========================================="

# Build the program
echo "Click... Clack... Building SBF binary..."
cd $PROGRAM_DIR
cargo build-sbf
cd ..

# Check if deploy binary exists
BINARY_PATH="./target/deploy/lazorkit_program.so"
if [ ! -f "$BINARY_PATH" ]; then
    echo "Error: Binary not found at $BINARY_PATH"
    exit 1
fi

# Deploy
echo "Deploying to network..."
solana program deploy \
    --url "$RPC_URL" \
    --keypair "$KEYPAIR" \
    "$BINARY_PATH"

echo "✅ Deployment complete!"
