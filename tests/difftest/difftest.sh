#!/usr/bin/env bash
# difftest.sh — rubash 与 GNU Bash 的差分测试驱动
#
# 对每个用例脚本, 分别用 GNU Bash 与 rubash 运行, 逐字节对比
# stdout / stderr / 退出码。与 GNU Bash 一致的用例标记 [PASS],
# 其余标记 [FAIL] 并打印两侧输出。
#
# 用法:
#   bash tests/difftest/difftest.sh                 # 跑全部用例
#   bash tests/difftest/difftest.sh 'case-0[1-5]*'  # 跑子集
#
# 环境变量:
#   BASH_RUNNER     GNU bash 路径 (默认 Git Bash)
#   RUBASH_RUNNER   rubash 二进制路径 (默认 target/debug/rubash.exe)

set -u

BASH_RUNNER="${BASH_RUNNER:-/c/Program Files/Git/bin/bash.exe}"
RUBASH_RUNNER="${RUBASH_RUNNER:-}"

CASES_DIR="$(cd "$(dirname "$0")" && pwd)/cases"
WORK="$(mktemp -d)"

if [ ! -f "$BASH_RUNNER" ] && ! command -v "$BASH_RUNNER" >/dev/null 2>&1; then
  echo "bash runner not found: $BASH_RUNNER" >&2
  exit 2
fi

if [ -z "$RUBASH_RUNNER" ]; then
  RUBASH_RUNNER="$(cd "$(dirname "$0")/../.." && pwd)/target/debug/rubash.exe"
fi
if [ ! -f "$RUBASH_RUNNER" ]; then
  echo "rubash runner not found: $RUBASH_RUNNER (build with: cargo build)" >&2
  exit 2
fi

pass=0
fail=0

for case_path in "$CASES_DIR"/${1:-case-*.sh}; do
  [ -f "$case_path" ] || continue
  name="$(basename "$case_path" .sh)"

  "$BASH_RUNNER" "$case_path" > "$WORK/bash.out" 2> "$WORK/bash.err"
  b_rc=$?
  "$RUBASH_RUNNER" "$case_path" > "$WORK/rubash.out" 2> "$WORK/rubash.err"
  r_rc=$?

  if [ "$b_rc" = "$r_rc" ] \
     && cmp -s "$WORK/bash.out" "$WORK/rubash.out" \
     && cmp -s "$WORK/bash.err" "$WORK/rubash.err"; then
    echo "[PASS] $name"
    pass=$((pass + 1))
    continue
  fi

  echo "[FAIL] $name (rc bash=$b_rc rubash=$r_rc)"
  fail=$((fail + 1))
  echo "  --- bash stdout:"; head -8 "$WORK/bash.out" | sed 's/^/    /'
  echo "  --- bash stderr:"; head -4 "$WORK/bash.err" | sed 's/^/    /'
  echo "  --- rubash stdout:"; head -8 "$WORK/rubash.out" | sed 's/^/    /'
  echo "  --- rubash stderr:"; head -4 "$WORK/rubash.err" | sed 's/^/    /'
done

rm -rf "$WORK"
echo "==== 汇总: PASS=$pass FAIL=$fail (共 $((pass + fail))) ===="
[ "$fail" = 0 ]
