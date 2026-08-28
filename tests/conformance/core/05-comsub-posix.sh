#!/usr/bin/env bash
# Test: POSIX command substitution `cmd`
# Root cause: POSIX command substitution parsing errors
set -euo pipefail

result=`echo hello`
if [[ "$result" == "hello" ]]; then
    echo "PASS"
else
    echo "FAIL: got '$result' expected 'hello'"
    exit 1
fi
