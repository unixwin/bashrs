#!/usr/bin/env bash
# Bare indexed-array operands in arithmetic use element zero (GNU expr.c).
x=(123 456)
reorder() {
  (( x[1] < x && (x=x[1], x[1]=$x) ))
  echo "${x[@]}"
}
reorder
x=(456 123)
reorder
