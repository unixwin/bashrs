#!/usr/bin/env bash
# case-21-subst-backslash-pattern — 参数替换 pattern 的 \\ 转义 (rubash#20 §10.6)
P2="C:\\work\\dir\\file.txt"
echo "A: ${P2//\\//}"
echo "B: ${P2//\\/}"
echo "C: ${P2##*\\}"
P="C:\\Users\\x\\repo"
Q="${P##*\\}"
echo "D: [$Q]"
