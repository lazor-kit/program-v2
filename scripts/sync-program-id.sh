#!/bin/bash

# Check if new Program ID is provided
if [ -z "$1" ]; then
    echo "Usage: $0 <new_program_id>"
    exit 1
fi

NEW_ID=$1

# Detect OLD_ID from assertions/src/lib.rs
OLD_ID=$(grep -oE "declare_id\!\(\"[A-Za-z0-9]+\"\)" assertions/src/lib.rs | sed -E 's/declare_id\!\(\"([A-Za-z0-9]+)\"\)/\1/')

if [ -z "$OLD_ID" ]; then
    echo "❌ Error: Could not detect current Program ID from assertions/src/lib.rs"
    exit 1
fi

if [ "$OLD_ID" == "$NEW_ID" ]; then
    echo "Program ID is already $NEW_ID. Skipping sync."
    exit 0
fi

echo "Syncing Program ID: $OLD_ID -> $NEW_ID"

# 1. Update Rust assertions
sed -i '' "s/$OLD_ID/$NEW_ID/g" assertions/src/lib.rs

# 2. Update SDK generation script
sed -i '' "s/$OLD_ID/$NEW_ID/g" sdk/solita-client/generate.mjs

# 3. Update SDK tests common configuration
sed -i '' "s/$OLD_ID/$NEW_ID/g" tests-sdk/tests/common.ts

# 4. Update validator start script in tests
sed -i '' "s/$OLD_ID/$NEW_ID/g" tests-sdk/package.json

# 5. Run SDK generation to update the TypeScript client
echo "Regenerating SDK..."
cd sdk/solita-client
node generate.mjs
cd ../..

echo "✓ Program ID synced across: Rust code, SDK, and Tests."
echo "✓ SDK regenerated with new address."
echo "Pro tip: Now run 'cargo build-sbf' to rebuild the program with the correct ID."
