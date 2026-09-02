#!/usr/bin/env bash
# Test: rsh builtin behavio
# Root cause: rsh builtin differs from bash
set -euo pipefail

# rsh should not exist as builtin in rubash
if type rsh 2>/dev/null | grep -q "builtin"; then
    echo "FAIL: rsh should not be a builtin"
    exit 1
else
    echo "PASS"
fi
