#!/usr/bin/env bash
# repro-10-paths.sh
# 问题: 路径形式转换不一致
#   bash: 变量保持原样; 外部命令能读任意 Windows 路径
#   rubash: 环境变量/赋值时反斜杠路径被转成 /c/...; 外部命令打不开 /c/... 路径
echo "== A: 环境变量读取 =="
echo "A1 LOCALAPPDATA=[${LOCALAPPDATA:-unset}]"
echo "A2 TEMP=[${TEMP:-unset}]"
echo "A3 HOME=[$HOME]"

echo "== B: 赋值转换 =="
P="C:\\Users\\caomengxuan\\repo"
echo "B1 P=[$P]"
echo "B2 P 长度=[${#P}]"

echo "== C: 外部命令读 /c/ 路径 =="
TXT="$HOME/wxsh-path-test.txt"
printf 'line1\n' > "$TXT"
C1="/c/Users/caomengxuan/wxsh-path-test.txt"
head -1 "$C1"
echo "C1 rc=$?"
head -1 "$TXT"
echo "C2 rc=$? (正斜杠 \$HOME 对照)"
rm -f "$TXT"

echo "== D: 参数替换结果再转换 =="
Q="${P//\\//}"
echo "D1 Q=[$Q]"
echo "== DONE =="
