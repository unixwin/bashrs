#!/usr/bin/env bash
# Test: POSIX pipe behavior
# Root cause: POSIX pipe behavior differs
set -euo pipefail

result=$(echo hello | cat)
if [[ "$result" == "hello" ]]; then
    echo "PASS"
else
    echo "FAIL: got '$result' expected 'hello'"
    exit 1
fi
