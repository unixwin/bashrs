#!/usr/bin/env bash
# repro-13-stderr-redirect.sh
# 问题: `2> file` stderr 重定向未生效 (stderr 直通终端, 文件为空)
#   bash: 文件内容为 "err-msg", 终端无输出
#   rubash: 终端显示 err-msg, 文件为空
echo "== A: 2> 重定向 =="
echo "err-msg" 2> "$HOME/repro-err.txt"
echo "A1 文件内容: [$(head -1 "$HOME/repro-err.txt" 2>/dev/null)]"
echo "A2 文件大小: [$(wc -c < "$HOME/repro-err.txt" 2>/dev/null)]"

echo "== B: 2>&1 合并 (对照) =="
echo "both-msg" > "$HOME/repro-both.txt" 2>&1
echo "B1 文件内容: [$(head -1 "$HOME/repro-both.txt")]"

echo "== C: 2>> 追加 =="
echo "err2" 2>> "$HOME/repro-err.txt"
echo "C1 追加后大小: [$(wc -c < "$HOME/repro-err.txt" 2>/dev/null)]"

rm -f "$HOME/repro-err.txt" "$HOME/repro-both.txt"
echo "== DONE =="
