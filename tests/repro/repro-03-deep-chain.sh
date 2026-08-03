#!/usr/bin/env bash
# repro-03-deep-chain.sh
# 问题: 多层函数 + 每层"赋值捕获命令替换" (out="$(...)") 时输出丢失
#   bash: 输出 deep-ok
#   rubash: 返回空 (注: 中间层用 printf 直接嵌套 $(...) 则正常)
set -u
level3() { printf 'deep-ok'; }
level2() { out="$(level3)"; printf '%s' "$out"; }
level1() { out="$(level2)"; printf '%s' "$out"; }
echo "A (3层赋值捕获链): [$(level1)]"
R="$(level1)"
echo "B (赋值捕获入口): [$R]"

# 变体: 4 层
level4() { out="$(level1)"; printf '%s' "$out"; }
echo "C (4层): [$(level4)]"
echo "== DONE =="
