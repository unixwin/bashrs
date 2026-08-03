#!/usr/bin/env bash
# case-13-arith — 算术 (对照, 应 PASS)
x=5
echo "A: $((x * 3))"
echo "B: $((x ** 2))"
echo "C: $((16#ff))"
echo "D: $((x++)) $x"
let "y = x + 1"
echo "E: $y"
echo "F: $(( (x > 3) ? 10 : 20 ))"
echo "G: $(( x & 3 ))"
