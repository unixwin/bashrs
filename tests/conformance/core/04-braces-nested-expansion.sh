#!/usr/bin/env bash
# Test: nested brace expansion {a,b}{1,2}
# Root cause: nested brace expansion broken in rubash
set -euo pipefail

result=$(echo {a,b}{1,2})
expected="a1 a2 b1 b2"

if [[ "$result" == "$expected" ]]; then
    echo "PASS"
else
    echo "FAIL: got '$result' expected '$expected'"
    exit 1
fi
