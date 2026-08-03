#!/usr/bin/env bash
# 被调试脚本: 覆盖函数、循环、条件、管道、命令替换
echo "start"
x=1
f1() {
  local y=2
  echo "in f1 y=$y"
}
f1
for i in a b; do
  echo "loop $i"
done
if true; then
  echo "cond"
fi
echo "pipe: $(echo hi | tr a-z A-Z)"
echo "end"
