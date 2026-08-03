#!/usr/bin/env bash
# case-02-ansi-c-quoting — $'...' ANSI-C quoting (赋值形式, 双引号内不触发)
# 已知差异: 完全不支持 (rubash#20 §9.1)
X=$'\t'
echo "tab: [$X]"
Y=$'\n'
echo "nl-before${Y}nl-after"
Z=$'a\x41'
echo "hex: [$Z]"
W=$'it'\''s'
echo "dq: [$W]"
echo "esc: [$'x\ny']"
