#!/usr/bin/env bash
# Test: arithmetic error should report diagnostic and allow script to continue
# Root cause: arithmetic errors terminate early in rubash
set -uo pipefail

# Division by zero should report error but not crash the shell
x=$((1/0)) 2>/dev/null || true
echo "after error"
echo "script continued"
