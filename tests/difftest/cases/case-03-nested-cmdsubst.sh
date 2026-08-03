#!/usr/bin/env bash
# case-03-nested-cmdsubst — 嵌套命令替换
# 已知差异: $(f "$(g)") 返回空 (rubash#20 §2.2)
inner() { printf 'inner'; }
outer() { printf '%s:%s' "$1" "$(printf '%s' "$1" | tr a-z A-Z)"; }
echo "A: $(outer "$(inner)")"

mid() { printf 'mid'; }
echo "B: $(printf 'v=[%s]' "$(mid)")"

echo "C: $(printf 'n=%s' "$(printf 'deep')")"
