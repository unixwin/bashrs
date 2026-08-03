#!/usr/bin/env bash
# case-23-debug-traps — trap DEBUG/RETURN/EXIT (rubash#20 §6)
# 注意: 函数返回时的 RETURN trap 在 bash 5.2 不触发(5.3 修复, rubash 对齐 5.3),
#       此处用 source 场景对比(bash 5.2/5.3 均触发)。
echo "== A: DEBUG trap =="
trap 'echo DBG' DEBUG
:
trap - DEBUG
echo "== B: RETURN trap (source) =="
RET_SRC="$TMPDIR/ret-src-$$.sh"
echo "echo in-src" > "$RET_SRC"
trap 'echo RET' RETURN
. "$RET_SRC"
trap - RETURN
rm -f "$RET_SRC"
echo "== C: EXIT trap =="
trap 'echo EXIT-FIRED' EXIT
echo "before exit"
