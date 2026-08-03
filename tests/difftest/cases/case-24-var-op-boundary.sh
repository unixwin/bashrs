#!/usr/bin/env bash
# case-24: var-op 边界（& 特殊替换、嵌套 slice、组合）— 族 I 回归
unset w
v="hello world"
echo "1: ${v//o/X&Y}"
echo "2: ${v/lo/&}"
echo "3: ${v:${w:-4}}"
echo "4: ${v: -3:2}"
echo "5: ${v: -8:-2}"
echo "6: ${#v}"
echo "7: ${v:-${w:-nested}}"
echo "8: ${v%l*}"
echo "9: ${v##*l}"
echo "10: ${v:0:${#v}-6}"
