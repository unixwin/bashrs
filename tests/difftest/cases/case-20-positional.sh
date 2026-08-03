#!/usr/bin/env bash
# case-20-positional — 位置参数/参数默认值 (对照, 应 PASS)
set -- a b c
echo "A: $1 $#"
shift
echo "B: $1 $#"
u=""
echo "C: ${u:-default}"
v="${u:=assigned}"
echo "D: $v $u"
w="set"
echo "E: ${w:+yes}"
echo "F: ${w:?}"
echo "G: ${10:-ten}"
