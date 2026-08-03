#!/usr/bin/env bash
# repro-06-subst-values.sh
# 问题: 参数替换的替换值/模式中的特殊字符被 rubash 破坏
#   bash: 输出 a\nb / a<换行>b / abc
#   rubash: 反斜杠被删(\n->n)、换行丢失、[}] 被 } 提前终止
echo "== A: 替换值 \\n (应输出字面 \\n, 即两字符) =="
t="a{newline}b"
r="${t//\{newline\}/\n}"
echo "A: [$r]"

echo "== B: 替换值变量含换行 (bash 输出两行) =="
N="$(printf '\n')"
r2="${t//\{newline\}/$N}"
echo "B: [$r2]"

echo "== C: 删除后缀 [}] 字符类 (应删掉 }) =="
o="abc}"
echo "C: [${o%[}]}]"

echo "== D: 替换值 \\\\n 双反斜杠 =="
r3="${t//\{newline\}/\\n}"
echo "D: [$r3]"
echo "== DONE =="
