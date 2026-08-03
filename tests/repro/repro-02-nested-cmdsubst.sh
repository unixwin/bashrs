#!/usr/bin/env bash
# repro-02-nested-cmdsubst.sh
# 问题: 嵌套命令替换 $(f "$(g)") 返回空
#   bash: 输出 "inner:HELLO"
#   rubash: bundle 为空
inner() { printf 'inner'; }
outer() { printf '%s:%s' "$1" "$(printf '%s' "$1" | tr a-z A-Z)"; }
echo "A: [$(outer "$(inner)")]"

# 变体: 函数参数里嵌命令替换
outer2() { printf 'v=[%s]' "$1"; }
echo "B: [$(outer2 "$(printf 'x')")]"

# 变体: 数组元素里嵌命令替换
echo "C: [$(printf 'arr=%s' "$(printf 'y')")]"
