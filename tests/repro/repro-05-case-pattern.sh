#!/usr/bin/env bash
# repro-05-case-pattern.sh
# 问题: case 模式不支持变量展开与转义字符
#   bash: 全部 MATCH
#   rubash: `"$q"*)` 与 `\"*)` 导致 case 结构解析失败(裸命令报错)
q='"'
v='"quoted"'
echo "== A: 字面模式 (应 MATCH) =="
case "$v" in
  '"'*)
    echo "A MATCH"
    ;;
esac

echo "== B: 变量模式 (bash 应 MATCH) =="
case "$v" in
  "$q"*)
    echo "B MATCH"
    ;;
esac

echo "== C: 转义模式 (bash 应 MATCH) =="
case "$v" in
  \"*)
    echo "C MATCH"
    ;;
esac

echo "== D: case 内嵌套 case 字面 (应 OK) =="
case "x" in
  x)
    case "y" in
      y) echo "D NESTED OK" ;;
    esac
    ;;
esac
echo "== DONE =="
