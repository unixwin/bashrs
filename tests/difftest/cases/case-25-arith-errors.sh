#!/usr/bin/env bash
# case-25: 算术错误码传播 + 进制字面量 — 族 G 回归
# 错误码的 rc/stderr 语义在手工用例验证；此处保留无 stderr 差异的 stdout 用例。
echo "base16: $((16#ff))"
echo "base2: $((2#1010))"
echo "base8: $((8#17))"
echo "base64: $((64#10))"
echo "ok: $((1+2))"
echo "after: ok"
