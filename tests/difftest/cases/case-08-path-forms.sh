#!/usr/bin/env bash
# case-08-path-forms — 路径形式矩阵 (输出避免打印 $HOME 原文, 用 basename/相对)
# 已知差异: 反斜杠路径赋值/环境变量被转 /c/ (rubash#20 §4.x)
echo "== 赋值后取 basename (路径形态不影响) =="
P1="C:/work/dir/file.txt"
P2="C:\\work\\dir\\file.txt"
P3="/c/work/dir/file.txt"
echo "A: ${P1##*/}"
echo "B: ${P2##*/}"
echo "C: ${P3##*/}"
echo "== 字符串替换 (斜杠方向) =="
echo "D: ${P2//\\//}"
echo "== test -f 用 \$HOME 相对 (应一致) =="
F=".winuxshrc"
[ -f "$HOME/$F" ] && echo "E: home-file-exists" || echo "E: missing"
echo "== cd 到 \$HOME 取 basename =="
cd "$HOME" 2>/dev/null
echo "F: $(basename "$PWD")"
