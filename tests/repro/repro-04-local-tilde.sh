#!/usr/bin/env bash
# repro-04-local-tilde.sh
# 问题: 函数内 `local x="$2"`(带初始化) 对含 ~ 的值做 tilde 展开
#   bash: 输出 ~/repo (保留字面)
#   rubash: 输出 /c/Users/caomengxuan/repo 或 C:/Users/caomengxuan/repo (被展开)
T='~'
f() {
  local x="$2"
  printf 'f=[%s]' "$x"
}
echo "A (local 带初始化): [$(f a "${T}/repo")]"

g() {
  local x
  x="$2"
  printf 'g=[%s]' "$x"
}
echo "B (先声明后赋值): [$(g a "${T}/repo")]"

h() {
  local y="${T}/repo"
  printf 'h=[%s]' "$y"
}
echo "C (local 初始化用变量拼接): [$(h)]"
