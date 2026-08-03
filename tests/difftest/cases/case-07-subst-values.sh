#!/usr/bin/env bash
# case-07-subst-values — 参数替换替换值/模式矩阵
# 已知差异: 反斜杠删除/换行丢失/[}] 字符类 (rubash#20 §2.5 §2.6)
t="a{newline}b"
echo "A: ${t//\{newline\}/X}"
echo "B: ${t//\{newline\}/\n}"
N="$(printf '\n')"
echo "C: [${t//\{newline\}/$N}]"
o="abc}"
echo "D: ${o%[}]}"
echo "E: ${o%?}"
s="Hello World"
echo "F: ${s/l/L}"
echo "G: ${s//l/L}"
echo "H: ${s#He}"
echo "I: ${s%%l*}"
