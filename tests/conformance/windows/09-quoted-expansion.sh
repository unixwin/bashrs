#!/usr/bin/env bash
# Test: Quoted variable expansion preserves spaces
set -euo pipefail

x="hello   world"
result="$x"
if [[ "$result" == "hello   world" ]]; then
    echo "PASS: quoted expansion preserves spaces"
else
    echo "FAIL: got '$result'"
    exit 1
fi
