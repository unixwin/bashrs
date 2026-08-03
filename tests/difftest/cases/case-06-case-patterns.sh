#!/usr/bin/env bash
# case-06-case-patterns — case 模式矩阵
# 已知差异: 变量/转义模式不匹配 (rubash#20 §2.4)
q='"'
v='"quoted"'
echo "== 字面模式 =="
case "$v" in
  '"'*) echo "A-MATCH" ;;
  *) echo "A-NO" ;;
esac
echo "== 变量模式 =="
case "$v" in
  "$q"*) echo "B-MATCH" ;;
  *) echo "B-NO" ;;
esac
echo "== 转义模式 =="
case "$v" in
  \"*) echo "C-MATCH" ;;
  *) echo "C-NO" ;;
esac
echo "== 普通字面 (对照) =="
case "abc" in
  a*) echo "D-MATCH" ;;
  *) echo "D-NO" ;;
esac
echo "== 嵌套 case =="
case "x" in
  x)
    case "y" in
      y) echo "E-NESTED" ;;
    esac
    ;;
esac
