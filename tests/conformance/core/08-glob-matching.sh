#!/usr/bin/env bash
# Test: glob matching behavio
# Root cause: glob matching differs from bash
set -uo pipefail

# Create test files in current di
cd "$(mktemp -d 2>/dev/null || echo .)"
touch rb_glob_a.txt rb_glob_b.txt rb_glob_c.txt 2>/dev/null || true

result=$(ls rb_glob_?.txt 2>/dev/null | wc -l)
if [[ "$result" -ge 3 ]]; then
    echo "PASS"
else
    echo "FAIL: glob matched $result files"
    rm -f rb_glob_?.txt 2>/dev/null || true
    exit 1
fi

# Cleanup
rm -f rb_glob_?.txt 2>/dev/null || true
