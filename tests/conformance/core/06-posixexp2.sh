#!/usr/bin/env bash
# Test: POSIX expression parsing
# Root cause: POSIX expression parsing errors in rubash
set -euo pipefail

# Test basic arithmetic in [[ ]]
x=5
if [[ $x -gt 3 ]]; then
    echo "PASS"
else
    echo "FAIL"
    exit 1
fi
