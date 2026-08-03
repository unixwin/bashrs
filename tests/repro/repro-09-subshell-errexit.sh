#!/usr/bin/env bash
# repro-09-subshell-errexit.sh
# 问题: 子 shell 内的 set -e 泄漏到父脚本 (父脚本被中断)
#   bash: 子 shell 退出码非零, 父脚本继续打印 "A rc=1"
#   rubash: 父脚本在子 shell 处直接终止, "A rc=" 不打印
echo "== A: 子 shell 内 set -e + false =="
( set -e; false; echo "A-should-not" )
echo "A rc=$? (父脚本应继续)"

echo "== B: 子 shell 内 set -e + 真命令 =="
( set -e; true )
echo "B rc=$?"

echo "== C: 子 shell 内 set -e + 条件豁免 =="
( set -e; if false; then echo "unreachable"; fi )
echo "C rc=$?"
echo "== DONE =="
