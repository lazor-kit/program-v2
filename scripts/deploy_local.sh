#!/bin/bash
set -e

# Compile programs
echo "🚧 Compiling programs..."
cargo build-sbf

# Project Root
PROJECT_ROOT=$(pwd)
TARGET_DEPLOY="$PROJECT_ROOT/target/deploy"
FIXTURES_DIR="$PROJECT_ROOT/ts-sdk/tests/fixtures/keypairs"

# Ensure fixtures directory exists
mkdir -p "$FIXTURES_DIR"

# Copy keypairs to fixtures
echo "🔑 Copying keypairs to fixtures..."
cp "$TARGET_DEPLOY/lazorkit_v2-keypair.json" "$FIXTURES_DIR/lazorkit-v2-keypair.json"
cp "$TARGET_DEPLOY/lazorkit_plugin_sol_limit-keypair.json" "$FIXTURES_DIR/sol-limit-plugin-keypair.json"
cp "$TARGET_DEPLOY/lazorkit_plugin_program_whitelist-keypair.json" "$FIXTURES_DIR/program-whitelist-plugin-keypair.json"

# Run TS deployment script
echo "🚀 Running TS deployment script..."
export ENABLE_DEPLOYMENT=true
export SOLANA_RPC_URL=http://localhost:8899

cd ts-sdk
if [ ! -d "node_modules" ]; then
    echo "📦 Installing npm dependencies..."
    npm install
fi

# We use the existing deploy.ts which handles the actual deployment logic
# using the keypairs we just copied (or checking target/deploy)
npm run deploy

echo "✅ Deployment complete!"
