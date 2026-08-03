#!/usr/bin/env bash
# case-09-redirects — 重定向矩阵
# 已知差异: 2> / 2>> stderr 重定向未生效 (rubash#20 §5.1)
echo "== A: stdout > =="
echo "out" > ./t-r.txt
echo "A: $(cat ./t-r.txt)"
echo "== B: stderr 2> =="
echo "err" 2> ./t-r2.txt
echo "B: [$(cat ./t-r2.txt 2>/dev/null)]"
echo "== C: 2>&1 =="
echo "both" > ./t-r3.txt 2>&1
echo "C: [$(cat ./t-r3.txt)]"
echo "== D: 2>> 追加 =="
echo "e1" 2> ./t-r4.txt
echo "e2" 2>> ./t-r4.txt
echo "D: [$(cat ./t-r4.txt 2>/dev/null)]"
echo "== E: fd 重定向 =="
exec 7> ./t-r5.txt
echo "fd7" >&7
exec 7>&-
echo "E: [$(cat ./t-r5.txt)]"
rm -f ./t-r.txt ./t-r2.txt ./t-r3.txt ./t-r4.txt ./t-r5.txt
