#!/usr/bin/env bash
# repro-11-cd-var-abs.sh
# 问题: cd 接"变量展开的绝对路径"时被当作相对路径拼接 PWD
#   bash: cd "$HOME" 后 PWD=C:/Users/caomengxuan
#   rubash: PWD=C:/Users/caomengxuan/repo/winuxsh/C:/Users/caomengxuan (拼接!)
echo "== A: cd 变量绝对路径 =="
cd "$HOME"
echo "A1 PWD=[$PWD]"

echo "== B: cd 字面 /c/ 绝对路径 (对照) =="
cd /c/Users/caomengxuan
echo "B1 PWD=[$PWD]"

echo "== C: cd 变量拼接绝对路径 =="
cd "$HOME/repo"
echo "C1 PWD=[$PWD]"

echo "== D: cd 相对路径 (对照) =="
cd scripts
echo "D1 PWD=[$PWD]"
cd ..
echo "D2 PWD=[$PWD]"
echo "== DONE =="
