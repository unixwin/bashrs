#!/usr/bin/env bash
# Test: mapfile with pipe
# Root cause: mapfile pipe behavior differs
set -euo pipefail

mapfile -t lines < <(echo -e "line1\nline2\nline3")
if [[ ${#lines[@]} -eq 3 ]]; then
    echo "PASS"
else
    echo "FAIL: got ${#lines[@]} lines expected 3"
    exit 1
fi
