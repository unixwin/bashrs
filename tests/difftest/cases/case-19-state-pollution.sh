#!/usr/bin/env bash
# case-19-state-pollution — 状态污染序列: 前置命令替换后复测 cd/printf/参数
# 已知差异: cd 拼接/printf 损坏/参数丢失 (rubash#20 §1.2 §1.3)
echo "== 前置: 多次命令替换 =="
A="$(printf 'a')"
B="$(printf '%s' "$A")"
C="$(printf '%s%s' "$A" "$B")"
echo "pre: $C"

echo "== 复测 1: cd 变量绝对路径 =="
cd "$HOME" 2>/dev/null
echo "cd1: $(basename "$PWD")"

echo "== 复测 2: printf 多参数 =="
printf 'P1=[%s] P2=[%s]\n' "x" "y"

echo "== 复测 3: 函数 4 参数 =="
arg4() {
  local a b c d
  a="$1"; b="$2"; c="$3"; d="$4"
  printf 'arg4: %s%s%s%s' "$a" "$b" "$c" "$d"
}
arg4 1 2 3 4
echo
echo "== DONE =="
