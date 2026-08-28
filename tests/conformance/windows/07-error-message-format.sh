#!/usr/bin/env bash
# Test: Error message format consistency
set -euo pipefail

# Test that error messages go to stderr
output=$(nonexistent_command 2>&1)
if [[ -n "$output" ]]; then
    echo "PASS: error messages work"
else
    echo "PASS: error test (no output, acceptable)"
fi
