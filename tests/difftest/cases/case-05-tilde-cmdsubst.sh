#!/usr/bin/env bash
# case-05-tilde-cmdsubst — 命令替换参数内字面 ~
# 已知差异: 双引号内字面 ~ 被展开 (rubash#20 §9.2)
echo "A: $(printf '%s' "~/repo")"
T='~'
echo "B: $(printf '%s' "${T}/repo")"
echo "C: $(printf '%s' "~")"
