#!/usr/bin/env bash
# GNU Bash Compatibility Test Suite
# rubash vs GNU Bash (WSL) - .right file based
set -euo pipefail

ROOT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")/../.." && pwd)"
RUBASH="$ROOT_DIR/target/debug/rubash.exe"
GNU_BASH="wsl bash"
TEST_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")/tests" && pwd)"
RIGHT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")/rights" && pwd)"
WORK_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")/work" && pwd)"

mkdir -p "$WORK_DIR"

# Colors
RED='\033[0;31m'
GREEN='\033[0;32m'
YELLOW='\033[1;33m'
CYAN='\033[0;36m'
NC='\033[0m'

# Counters
total=0
pass=0
fail=0
skip=0

# Parse args
TEST_NAME="${1:-all}"

usage() {
    echo "Usage: $0 [test-name|all|list]"
    echo ""
    echo "Examples:"
    echo "  $0              # Run all tests"
    echo "  $0 braces       # Run braces tests only"
    echo "  $0 list         # List all available tests"
    exit 0
}

[[ "$TEST_NAME" == "-h" || "$TEST_NAME" == "--help" ]] && usage

# List mode
if [[ "$TEST_NAME" == "list" ]]; then
    echo "Available tests:"
    for f in "$TEST_DIR"/*.sh; do
        [[ -f "$f" ]] || continue
        name=$(basename "$f" .sh)
        echo "  $name"
    done
    exit 0
fi

echo -e "${CYAN}=== GNU Bash Compatibility Test Suite ===${NC}"
echo "rubash: $RUBASH"
echo "GNU Bash (WSL): $GNU_BASH"
echo ""

# Generate .right files if missing
generate_rights() {
    local test_file="$1"
    local test_name=$(basename "$test_file" .sh)
    local right_file="$RIGHT_DIR/${test_name}.right"
    
    if [[ ! -f "$right_file" ]]; then
        echo -e "${YELLOW}GENERATING${NC} ${test_name}.right"
        # Copy test file to WSL and execute
        $GNU_BASH -c "cp /mnt/d/repo/rubash/$test_file /tmp/gen_test.sh && chmod +x /tmp/gen_test.sh && /tmp/gen_test.sh" > "$right_file" 2>/dev/null || true
    fi
}

# Run a single test
run_test() {
    local test_file="$1"
    local test_name=$(basename "$test_file" .sh)
    local right_file="$RIGHT_DIR/${test_name}.right"
    local rubash_out="$WORK_DIR/${test_name}.rubash.out"
    local diff_file="$WORK_DIR/${test_name}.diff"
    
    ((total++))
    
    # Check for SKIP marker
    if head -5 "$test_file" | grep -q "# SKIP"; then
        echo -e "${YELLOW}SKIP${NC}  $test_name"
        ((skip++))
        return 0
    fi
    
    # Generate .right if missing
    generate_rights "$test_file"
    
    # Skip if .right is empty
    if [[ ! -s "$right_file" ]]; then
        echo -e "${YELLOW}SKIP${NC}  $test_name (empty .right)"
        ((skip++))
        return 0
    fi
    
    # Run under rubash
    "$RUBASH" "$test_file" > "$rubash_out" 2>/dev/null || true
    
    # Normalize line endings
    sed -i 's/\r$//' "$rubash_out" 2>/dev/null || true
    
    # Compare with .right
    if diff -u "$right_file" "$rubash_out" > "$diff_file" 2>&1; then
        echo -e "${GREEN}PASS${NC}  $test_name"
        ((pass++))
    else
        echo -e "${RED}FAIL${NC}  $test_name"
        head -3 "$diff_file" | sed 's/^/  /'
        ((fail++))
    fi
}

# Main loop
if [[ "$TEST_NAME" == "all" ]]; then
    for test_file in "$TEST_DIR"/*.sh; do
        [[ -f "$test_file" ]] || continue
        run_test "$test_file"
    done
else
    test_file="$TEST_DIR/${TEST_NAME}.sh"
    if [[ -f "$test_file" ]]; then
        run_test "$test_file"
    else
        echo "Test not found: $TEST_NAME"
        exit 1
    fi
fi

# Summary
echo ""
echo -e "${CYAN}=== Results ===${NC}"
echo -e "${GREEN}Pass: $pass${NC}"
echo -e "${RED}Fail: $fail${NC}"
echo -e "${YELLOW}Skip: $skip${NC}"
echo "Total: $total"

[[ $fail -eq 0 ]] && exit 0 || exit 1
