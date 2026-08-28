#!/usr/bin/env bash
# Test: Variable export to child processes
set -euo pipefail

export TEST_VAR="hello_world"
result=$($RUBASH -c 'echo $TEST_VAR' 2>/dev/null || target/debug/rubash.exe -c 'echo $TEST_VAR')

if [[ "$result" == "hello_world" ]]; then
    echo "PASS: variable export works"
else
    echo "FAIL: got '$result' expected 'hello_world'"
    exit 1
fi
