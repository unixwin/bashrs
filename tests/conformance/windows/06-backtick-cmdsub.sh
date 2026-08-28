#!/usr/bin/env bash
# Test: Backtick command substitution equivalence
set -euo pipefail

# Backticks and $() should produce same result
result_bt=`echo hello`
result_dq=$(echo hello)

if [[ "$result_bt" == "$result_dq" ]]; then
    echo "PASS: backtick and dollar-parens equivalent"
else
    echo "FAIL: backtick='$result_bt' dollar-parens='$result_dq'"
    exit 1
fi
