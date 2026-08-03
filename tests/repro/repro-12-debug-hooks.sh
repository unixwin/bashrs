#!/usr/bin/env bash
# repro-12-debug-hooks.sh
# 问题: 调试钩子部分缺失/异常
#   bash: PS4 自定义生效; trap DEBUG/RETURN/EXIT 触发; BASH_VERSINFO 存在
#   rubash: PS4 不生效; DEBUG/RETURN/EXIT trap 不触发; BASH_VERSINFO 缺失
echo "== A: PS4 自定义 =="
PS4='DBG> '
set -x
X=1
set +x
echo "A rc=$?"

echo "== B: trap DEBUG =="
trap 'echo "DEBUG-FIRED"' DEBUG
echo "B after trap DEBUG"
trap - DEBUG

echo "== C: trap RETURN =="
tf() { trap 'echo "RETURN-FIRED"' RETURN; echo "C in fn"; }
tf
echo "C after fn"

echo "== D: trap ERR (对照, 应触发) =="
trap 'echo "ERR-FIRED"' ERR
false
trap - ERR
echo "D after false"

echo "== E: BASH_VERSINFO =="
echo "E1 BASH_VERSION=[${BASH_VERSION:-unset}]"
echo "E2 BASH_VERSINFO=[${BASH_VERSINFO[*]:-unset}]"

echo "== F: 栈变量 (对照, 应工作) =="
t2() {
  echo "F1 FUNCNAME=[${FUNCNAME[*]}]"
  echo "F2 BASH_SOURCE=[${BASH_SOURCE[*]}]"
  echo "F3 LINENO=[$LINENO]"
}
t2
echo "== DONE =="
