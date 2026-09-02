#!/usr/bin/env bash
# repro-08-pipe-builtin.sh
# 问题: 管道无法运行特殊内建命令
#   bash: 输出 set -o 列表(约27行)
#   rubash: "pipeline command could not execute: builtin command not found"
echo "== A: set -o | head =="
set -o 2>&1 | head -3
echo "A rc=$?"

echo "== B: 其他特殊内建进管道 =="
set | head -2
echo "B rc=$?"

echo "== C: export 进管道 =="
export FOO=ba
export | head -2
echo "C rc=$?"

echo "== D: 普通内建进管道 (printf 应正常) =="
printf 'x\ny\n' | head -1
echo "D rc=$?"
echo "== DONE =="
