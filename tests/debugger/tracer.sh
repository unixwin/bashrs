#!/usr/bin/env bash
# tracer.sh — 最小 bash 调试器/跟踪器（模拟 bashdb 的 DEBUG trap 钩子机制）
# 用途: 验证 shell 实现是否支持外部调试工具依赖的标准钩子:
#   - DEBUG trap 在每条命令前触发
#   - trap action 中可读 FUNCNAME/BASH_SOURCE/BASH_LINENO/LINENO
#   - trap action 中的命令不再递归触发 DEBUG
# 用法: <shell> tracer.sh <被调试脚本>
set -u

TRACE_OUT="${TRACE_OUT:-}"
TRACE_FD=2
if [ -n "$TRACE_OUT" ]; then
  exec {TRACE_FD}>"$TRACE_OUT"
fi

trace_hook() {
  local fn="${FUNCNAME[1]:-main}"
  local src="${BASH_SOURCE[1]:-}"
  local line="${BASH_LINENO[0]:-0}"
  echo "TRACE [$fn] $src:$line ${BASH_COMMAND:-}" >&$TRACE_FD
  return 0
}

trap 'trace_hook' DEBUG

# 执行被调试脚本
. "$1"
