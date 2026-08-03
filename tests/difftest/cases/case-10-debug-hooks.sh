#!/usr/bin/env bash
# case-10-debug-hooks — 调试钩子矩阵
# 已知差异: PS4/trap DEBUG/RETURN/EXIT/BASH_VERSINFO (rubash#20 §6)
echo "== A: PS4 =="
PS4='DBG> '
set -x
X=1
set +x
echo "== B: trap ERR (对照, 应触发) =="
trap 'echo ERR-FIRED' ERR
false
trap - ERR
echo "== C: BASH_VERSINFO =="
echo "VI: ${BASH_VERSINFO[*]:-unset}"
echo "== D: 栈变量 (对照) =="
t2() {
  echo "FN: ${FUNCNAME[*]}"
  echo "LN: $LINENO"
}
t2
