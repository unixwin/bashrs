#!/usr/bin/env bash
# Test: unclosed [[ should give syntax error
# Root cause: conditional expression errors in rubash
set -euo pipefail

[[ -z ]] && echo "true" || echo "false"
echo "rc=$?"
