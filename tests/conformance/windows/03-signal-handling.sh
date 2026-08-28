#!/usr/bin/env bash
# Test: Signal handling on Windows
set -euo pipefail

# Test SIGCHLD handling (was causing fatal 128+status)
trap "" CHLD
sleep 0.1 &
wait
echo "PASS: SIGCHLD handled"
