#!/usr/bin/env bash
# Test: array invalid index should not terminate script
# Root cause: array errors terminate early in rubash
set -euo pipefail

declare -a arr
arr[hello]=1
echo "after error rc=$?"
echo "script continued"
