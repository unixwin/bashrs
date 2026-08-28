#!/usr/bin/env bash
# Test: completion exit code
# Root cause: completion exit code differs from bash
set -euo pipefail

# Test that complete -p works
result=$(complete -p cd 2>&1)
if [[ $? -eq 0 ]] || [[ -n "$result" ]]; then
    echo "PASS"
else
    echo "FAIL"
    exit 1
fi
