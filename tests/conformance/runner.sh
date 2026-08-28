#!/usr/bin/env bash
# Rubash Conformance Test Runner
# rubash is the authoritative reference for Windows shell semantics
set -euo pipefail

ROOT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")/../.." && pwd)"
RUBASH="$ROOT_DIR/target/debug/rubash.exe"
CONFORMANCE_DIR="$ROOT_DIR/tests/conformance"
CATEGORY="${1:-all}"
COMPARE_BASH="${RUBASH_COMPARE_BASH:-0}"

# Colors
RED='\033[0;32m'
GREEN='\033[0;32m'
YELLOW='\033[1;33m'
NC='\033[0m'

pass=0
fail=0
skip=0

run_test() {
    local test_file="$1"
    local test_name="$(basename "$test_file" .sh)"
    
    # Check for SKIP marker
    if grep -q '# SKIP' "$test_file" 2>/dev/null; then
        echo -e "${YELLOW}SKIP${NC} $test_name"
        ((skip++)) || true
        return 0
    fi
    
    # Run the test under rubash
    local rb_exit=0
    local rb_output
    rb_output=$("$RUBASH" "$test_file" 2>&1) || rb_exit=$?
    
    if [[ "$rb_exit" -eq 0 ]]; then
        echo -e "${GREEN}PASS${NC} $test_name"
        ((pass++)) || true
    else
        echo -e "${RED}FAIL${NC} $test_name (exit=$rb_exit)"
        echo "  Output: $(echo "$rb_output" | head -3)"
        ((fail++)) || true
    fi
    
    # Compare with GNU Bash if requested
    if [[ "$COMPARE_BASH" == "1" ]]; then
        local bash_path="D:/Git/bin/bash.exe"
        if [[ -x "$bash_path" ]]; then
            local bash_exit=0
            local bash_output
            bash_output=$("$bash_path" "$test_file" 2>&1) || bash_exit=$?
            if [[ "$rb_exit" != "$bash_exit" ]] || [[ "$rb_output" != "$bash_output" ]]; then
                echo "  ${YELLOW}DIFF vs GNU Bash: rb_exit=$rb_exit bash_exit=$bash_exit${NC}"
            fi
        fi
    fi
}

echo "=== Rubash Conformance Test Suite ==="
echo "Reference: rubash (not GNU Bash)"
echo ""

case "$CATEGORY" in
    core)
        echo "--- Core Bug Regressions ---"
        for t in "$CONFORMANCE_DIR/core/"*.sh; do
            [[ -f "$t" ]] && run_test "$t"
        done
        ;;
    windows)
        echo "--- Windows Semantics ---"
        for t in "$CONFORMANCE_DIR/windows/"*.sh; do
            [[ -f "$t" ]] && run_test "$t"
        done
        ;;
    all)
        for dir in core windows; do
            echo "--- $dir ---"
            for t in "$CONFORMANCE_DIR/$dir/"*.sh; do
                [[ -f "$t" ]] && run_test "$t"
            done
        done
        ;;
    *)
        echo "Usage: $0 [core|windows|all]"
        exit 1
        ;;
esac

echo ""
echo "=== Results ==="
echo -e "${GREEN}Pass: $pass${NC}"
echo -e "${RED}Fail: $fail${NC}"
echo -e "${YELLOW}Skip: $skip${NC}"

[[ $fail -eq 0 ]] && exit 0 || exit 1
