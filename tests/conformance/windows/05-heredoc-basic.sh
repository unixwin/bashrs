#!/usr/bin/env bash
# Test: Heredoc functionality
set -euo pipefail

result=$(cat << EOF
line1
line2
line3
EOF
)

expected="line1
line2
line3"

if [[ "$result" == "$expected" ]]; then
    echo "PASS: heredoc works"
else
    echo "FAIL: heredoc mismatch"
    exit 1
fi
