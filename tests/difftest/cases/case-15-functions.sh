#!/usr/bin/env bash
# case-15-functions — 函数特性 (对照, 应 PASS)
myfunc() {
  local lv="local-val"
  echo "A: $lv"
  return 7
}
myfunc
echo "B: rc=$?"
echo "C: ${lv:-unset}"
export -f myfunc 2>/dev/null && echo "D: export-f-ok"
declare -f myfunc >/dev/null 2>&1 && echo "E: declare-f-ok"
f2() { echo "F: $1/$2"; }
f2 alpha beta
f3() { local x=1; local y; y=2; echo "G: $x$y"; }
f3
