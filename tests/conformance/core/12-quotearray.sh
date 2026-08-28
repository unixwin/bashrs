#!/usr/bin/env bash
# Test: quoted array expansion
# Root cause: quoted array expansion differs
set -euo pipefail

declare -a arr=("hello world" "foo bar")
result="${arr[0]}"
if [[ "$result" == "hello world" ]]; then
    echo "PASS"
else
    echo "FAIL: got '$result' expected 'hello world'"
    exit 1
fi
