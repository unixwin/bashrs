#!/usr/bin/env bash
# repro-01-heredoc-context.sh
# 问题: heredoc 变量展开依赖接收者
#   bash: 两种上下文都展开 $HOME
#   rubash: `cat > f <<EOF` 展开; `while read <<EOF` / `done <<EOF` 不展开(字面)
echo "== case A: heredoc 重定向到外部命令 (bash 应展开) =="
cat > "$HOME/repro-hd-a.txt" <<EOF
expanded $HOME
EOF
echo "A: [$(head -1 "$HOME/repro-hd-a.txt")]"

echo "== case B: heredoc 喂给 while read (bash 应展开) =="
while IFS= read -r line; do
  echo "B: [$line]"
  break
done <<EOF
expanded $HOME
EOF

echo "== case C: heredoc 喂给 for 循环? (bash 应展开) =="
for l in $(cat <<EOF
expanded $HOME
EOF
); do echo "C: [$l]"; break; done

rm -f "$HOME/repro-hd-a.txt"
