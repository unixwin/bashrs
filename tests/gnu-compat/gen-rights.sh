#!/usr/bin/env bash
# Generate .right files from GNU Bash (WSL)
set -euo pipefail

TEST_DIR="tests/gnu-compat/tests"
RIGHT_DIR="tests/gnu-compat/rights"

mkdir -p "$RIGHT_DIR"

for test_file in "$TEST_DIR"/*.sh; do
    [[ -f "$test_file" ]] || continue
    test_name=$(basename "$test_file" .sh)
    right_file="$RIGHT_DIR/${test_name}.right"
    
    echo -n "Generating $test_name.right... "
    
    # Run under GNU Bash via WSL and capture output
    # Copy test file to WSL temp location
    wsl bash -c "cp /mnt/d/repo/rubash/$test_file /tmp/gen_test.sh"
    wsl bash -c "chmod +x /tmp/gen_test.sh"
    wsl bash -c "/tmp/gen_test.sh > /tmp/gen_out.txt 2>/tmp/gen_err.txt" || true
    
    # Copy output back
    wsl bash -c "cp /tmp/gen_out.txt /mnt/d/repo/rubash/$right_file"
    
    echo "done"
done

echo "Generated .right files in $RIGHT_DIR"
