#!/usr/bin/env bash
# case-04-fn-chain-assign — 多层函数 + 赋值捕获命令替换
# 已知差异: 深层链输出丢失 (rubash#20 §1.1, 间歇/状态依赖)
set -u
lvl3() { printf 'deep-ok'; }
lvl2() { out="$(lvl3)"; printf '%s' "$out"; }
lvl1() { out="$(lvl2)"; printf '%s' "$out"; }
echo "A: $(lvl1)"
R="$(lvl1)"
echo "B: $R"
R2="$(lvl1)"
echo "C: $R2"
