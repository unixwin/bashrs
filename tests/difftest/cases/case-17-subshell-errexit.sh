#!/usr/bin/env bash
# case-17-subshell-errexit — 子 shell 内 set -e
# 已知差异: errexit 泄漏到父脚本 (rubash#20 §3.2)
echo "A-start"
( set -e; false; echo "A-should-not" )
echo "A-rc=$?"
( set -e; true )
echo "B-rc=$?"
echo "== DONE =="
