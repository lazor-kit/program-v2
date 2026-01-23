#!/bin/bash
set -e

if [ -z "$PROGRAM_ID" ]; then
    echo "Error: PROGRAM_ID environment variable is missing."
    echo "Usage: PROGRAM_ID=<program_id> ./test_rpc.sh"
    exit 1
fi

echo "Running Integration Tests against Program: $PROGRAM_ID"
cargo run -p lazorkit-tests-e2e
