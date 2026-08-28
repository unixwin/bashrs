#!/usr/bin/env bash
# Test: POSIX compliance
# Root cause: POSIX compliance differences
set -euo pipefail

# Test POSIX variable expansion
x="hello world"
result="${x#* }"
if [[ "$result" == "world" ]]; then
    echo "PASS"
else
    echo "FAIL: got '$result' expected 'world'"
    exit 1
fi
