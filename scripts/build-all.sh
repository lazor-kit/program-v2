#!/bin/bash
# Build the Rust program for a chosen cluster, derive the program ID from
# the resulting keypair, regenerate IDL.
#
# Usage:
#   ./scripts/build-all.sh devnet     # builds with --features devnet (FLb7...)
#   ./scripts/build-all.sh mainnet    # builds with --features mainnet (LazorjRF... — slot shared with lazorkit-protocol)
#
# After this script the .so + keypair live at target/deploy/. Deploy with:
#   solana program deploy target/deploy/lazorkit_program.so -u <cluster>
#
# The TypeScript SDK lives in the sibling lazorkit-protocol repo at
# sdk/sdk-legacy/ and is published to npm as @lazorkit/sdk-legacy. No
# SDK regeneration step here — the SDK is hand-written and works
# unchanged against either cluster (it probes ProtocolConfig at runtime).
set -e

CLUSTER=$1
ROOT_DIR=$(pwd)
PROGRAM_DIR="$ROOT_DIR/program"

if [ "$CLUSTER" != "mainnet" ] && [ "$CLUSTER" != "devnet" ]; then
    echo "Usage: $0 <mainnet|devnet>"
    exit 1
fi

echo "--- 🚀 LazorKit build (cluster: $CLUSTER) ---"

# Step 1: Build Rust Program with the chosen cluster feature.
# This embeds the right declare_id! at compile time via assertions/src/lib.rs.
echo "[1/2] Building Rust Program (cargo build-sbf --features $CLUSTER)..."
cd "$PROGRAM_DIR"
cargo build-sbf --features "$CLUSTER"

# Step 2: Generate IDL using Shank, picking the program ID from the keypair
# the build emitted at target/deploy/lazorkit_program-keypair.json.
echo "[2/2] Generating IDL..."
PROGRAM_ID=$(solana-keygen pubkey ../target/deploy/lazorkit_program-keypair.json)
echo "  resolved program ID: $PROGRAM_ID"
if command -v shank &> /dev/null; then
    shank idl -o . --out-filename idl.json -p "$PROGRAM_ID"
else
    echo "❌ Error: shank CLI not found. Please install it with 'cargo install shank-cli'."
    exit 1
fi

echo "--- ✅ Done ($CLUSTER) ---"
echo "Deploy:  solana program deploy ../target/deploy/lazorkit_program.so -u $([ "$CLUSTER" = "mainnet" ] && echo m || echo d)"
