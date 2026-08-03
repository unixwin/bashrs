#!/usr/bin/env bash
# repro-07-crash-stackoverflow.sh
# 问题: `${P##*\\}` 参数展开导致 rubash 栈溢出崩溃 (0xC00000FD)
#   bash: 输出 repo (删除最长前缀 `*\`)
#   rubash: 崩溃 "thread 'main' has overflowed its stack"
echo "== A: 删除最长反斜杠前缀 =="
P="C:\\Users\\caomengxuan\\repo"
Q="${P##*\\}"
echo "A: [$Q]"

echo "== B: 删除最短反斜杠前缀 =="
R="${P#*\\}"
echo "B: [$R]"
echo "== DONE =="
