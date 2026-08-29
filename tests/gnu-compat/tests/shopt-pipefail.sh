#!/usr/bin/env bash
# Test: shopt-pipefail (actually set -o pipefail)
set -o pipefail
true | false
echo $?
