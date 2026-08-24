#!/usr/bin/env bash
# Minimal fixture used by the Rubash getopts_long compatibility probe.
getopts_long() {
  local _optstring=$1
  local _name=$2
  local _option=$3
  local _arg=$4
  local _unused=$5
  local _argv=$6
  printf -v "${_name}" '%s' "${_option#--}"
  OPTLARG=''
  OPTLIND=2
  return 0
}
