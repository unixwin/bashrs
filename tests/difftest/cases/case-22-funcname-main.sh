#!/usr/bin/env bash
# case-22-funcname-main — FUNCNAME/BASH_SOURCE 栈含顶层 main (rubash#20 §10.5)
t2() { echo "FN: ${FUNCNAME[*]}"; echo "SRC: ${BASH_SOURCE[*]}"; echo "LN: $LINENO"; }
t2
f1() { f2; }
f2() { echo "FN2: ${FUNCNAME[*]}"; }
f1
