#!/usr/bin/env bash
# case-14-cond — [[ ]] 矩阵 (对照, 应 PASS)
[[ "abc" == a* ]] && echo "A: glob"
[[ "abc" =~ ^a.c$ ]] && echo "B: regex"
[[ -n "x" && "y" == y ]] && echo "C: and"
[[ 5 -gt 3 ]] && echo "D: arith"
[[ -d "$HOME" ]] && echo "E: -d"
[[ -z "" ]] && echo "F: -z"
[[ "a b" == *" "* ]] && echo "G: quoted-glob"
[[ "ABC" == "abc" ]] && echo "H: case" || echo "H: case-sensitive"
