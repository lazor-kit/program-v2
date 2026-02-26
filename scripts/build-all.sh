#!/bin/bash

# Configuration
PROGRAM_ID=$1
ROOT_DIR=$(pwd)
PROGRAM_DIR="$ROOT_DIR/program"
SDK_DIR="$ROOT_DIR/sdk/lazorkit-ts"

if [ -z "$PROGRAM_ID" ]; then
    echo "Usage: $0 <new_program_id>"
    echo "Example: $0 2m47smrvCRpuqAyX2dLqPxpAC1658n1BAQga1wRCsQiT"
    exit 1
fi

echo "--- 🚀 Starting LazorKit Full Sync Workflow ---"

# Step 1: Update Program ID everywhere
echo "[1/4] Syncing Program ID to $PROGRAM_ID..."
./scripts/sync-program-id.sh "$PROGRAM_ID"

# Step 2: Build Rust Program
echo "[2/4] Building Rust Program..."
cargo build-sbf

# Step 3: Generate IDL using Shank
echo "[3/4] Generating IDL..."
cd "$PROGRAM_DIR"
# Assuming shank is installed. If not, this will fail with a clear msg.
if command -v shank &> /dev/null; then
    shank idl -o . --out-filename idl.json -p "$PROGRAM_ID"
else
    echo "❌ Error: shank CLI not found. Please install it with 'cargo install shank-cli'."
    exit 1
fi

# Step 4: Regenerate SDK with Codama
echo "[4/4] Regenerating Codama SDK..."
cd "$SDK_DIR"
npm run generate

echo "--- ✅ All Done! ---"
echo "Next: Deploy your program using 'solana program deploy program/target/deploy/lazorkit_program.so -u d'"
