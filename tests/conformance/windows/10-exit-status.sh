#!/usr/bin/env bash
# Test: Exit status propagation with set -e
set -uo pipefail

# false with set -e should cause exit
code=0
false || code=$?

if [[ "$code" -ne 0 ]]; then
    echo "PASS: false returns non-zero"
else
    echo "FAIL: false should return non-zero"
    exit 1
fi
