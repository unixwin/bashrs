#!/usr/bin/env bash
# case-11-arrays — 数组/关联数组 (对照, 应 PASS)
arr=(one two three)
echo "A: ${#arr[@]} ${arr[1]}"
arr+=(four)
echo "B: ${#arr[@]}"
arr[5]=six
echo "C: ${#arr[@]} ${arr[*]}"
unset 'arr[1]'
echo "D: ${#arr[@]} ${arr[*]}"
declare -A assoc
assoc[k1]=v1
assoc[k2]=v2
echo "E: ${!assoc[*]}"
echo "F: ${assoc[k1]} ${#assoc[@]}"
