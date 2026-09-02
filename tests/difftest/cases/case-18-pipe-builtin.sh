#!/usr/bin/env bash
# case-18-pipe-builtin — 特殊内建进管道
# 已知差异: 管道无法运行特殊内建 (rubash#20 §3.1)
echo "== A: 普通内建进管道 (对照) =="
printf 'x\ny\n' | head -n 1
echo "A-rc=$?"
echo "== B: 特殊内建进管道 =="
set -o 2>&1 | head -n 2
echo "B-rc=$?"
echo "== C: export 进管道 =="
export FOO=ba
export | head -n 2
echo "C-rc=$?"
echo "== DONE =="
