#!/usr/bin/env bash
# case-01-heredoc-receivers — heredoc 变量展开 × 接收者矩阵
# 已知差异: while read / done <<EOF 不展开 (rubash#20 §2.1)
V="hello"
echo "== cat > file <<EOF =="
cat > ./t-hd.txt <<EOF
value $V
EOF
cat ./t-hd.txt
rm -f ./t-hd.txt

echo "== while read <<EOF =="
while IFS= read -r l; do
  echo "got: $l"
done <<EOF
value $V
EOF

echo "== for in \$(cat <<EOF) =="
for w in $(cat <<EOF
a $V b
EOF
); do echo "w: $w"; done

echo "== read -r <<< here-string (对照) =="
read -r hs <<< "hs $V"
echo "hs: $hs"
