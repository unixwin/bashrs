#!/usr/bin/env bash
# Test: /dev/null and device file handling
set -uo pipefail

# Test /dev/null exists and works
echo "test" > /dev/null
result=$(echo "hello" > /dev/null && echo "ok")
if [[ "$result" == "ok" ]]; then
    echo "PASS: /dev/null works"
else
    echo "FAIL: /dev/null broken"
    exit 1
fi
