#!/usr/bin/env bash
set -uo pipefail
ROOT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")/../.." && pwd)"
RUBASH="$ROOT_DIR/target/debug/rubash.exe"
UPSTREAM_DIR="$ROOT_DIR/third_party/bash/tests"
RIGHT_DIR="$ROOT_DIR/tests/gnu-compat/upstream-rights"
WORK_DIR="$ROOT_DIR/tests/gnu-compat/upstream-work"
HELPERS="$ROOT_DIR/tests/gnu-compat/helpers-win"
WSL_TEMP="/tmp/rubash-upstream-test"
# Convert Git Bash path to WSL path
to_wsl() { echo "$1" | sed "s|^/\([a-z]\)/|/mnt/\1/|;s|^\([A-Z]\):/|/mnt/\L\1/|"; }
WSL_UP=$(to_wsl "$UPSTREAM_DIR")
WSL_RI=$(to_wsl "$RIGHT_DIR")
mkdir -p "$RIGHT_DIR" "$WORK_DIR"
total=0; pass=0; fail=0; skip=0; gen=0
echo "=== Upstream Bash Tests ==="
echo
echo "Preparing WSL..."
wsl bash -c "rm -rf $WSL_TEMP && mkdir -p $WSL_TEMP"
for f in "$UPSTREAM_DIR"/*; do
    [ -f "$f" ] || continue
    bn=$(basename "$f")
    wsl bash -c "cp $WSL_UP/$bn $WSL_TEMP/$bn" 2>/dev/null || true
done
wsl bash -c "cd $WSL_TEMP && sed -i 's/\r$//' *.tests *.sub 2>/dev/null" 2>/dev/null || true
wsl bash -c "chmod +x $WSL_TEMP/recho $WSL_TEMP/zecho $WSL_TEMP/printenv 2>/dev/null" 2>/dev/null || true
wsl_count=$(wsl bash -c "ls $WSL_TEMP/*.tests 2>/dev/null | wc -l")
echo "  WSL files: $wsl_count"
echo "Ready."
echo
for test_file in "$UPSTREAM_DIR"/*.tests; do
    [ -f "$test_file" ] || continue
    name=$(basename "$test_file" .tests)
    right_file="$RIGHT_DIR/${name}.right"
    rubash_out="$WORK_DIR/${name}.rubash.out"
    diff_file="$WORK_DIR/${name}.diff"
    total=$((total + 1))
    if [ ! -f "$right_file" ] || [ ! -s "$right_file" ]; then
        echo -n "GEN   $name ... "
        wsl bash -c "cd $WSL_TEMP && PATH=$WSL_TEMP:/usr/local/bin:/usr/bin:/bin /usr/bin/bash $WSL_TEMP/${name}.tests > /tmp/_up_out.txt 2>/tmp/_up_err.txt" 2>/dev/null || true
        wsl bash -c "cp /tmp/_up_out.txt $WSL_RI/${name}.right" 2>/dev/null
        gen=$((gen + 1))
        echo "done"
    fi
    if [ ! -s "$right_file" ]; then
        echo "SKIP  $name (empty .right)"
        skip=$((skip + 1))
        continue
    fi
    PATH="$HELPERS:$PATH" "$RUBASH" "$test_file" > "$rubash_out" 2>/dev/null || true
    if diff -u "$right_file" "$rubash_out" > "$diff_file" 2>&1; then
        echo "PASS  $name"
        pass=$((pass + 1))
    else
        echo "FAIL  $name"
        fail=$((fail + 1))
    fi
done
echo
echo "=== Results ==="
echo "Pass: $pass"
echo "Fail: $fail"
echo "Skip: $skip"
echo "Generated: $gen .right files"
echo "Total: $total"
[ $fail -eq 0 ] && exit 0 || exit 1
