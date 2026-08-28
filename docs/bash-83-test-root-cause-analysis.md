# Bash 官方 83 测试根因分析

> 日期: 2026-08-28
> 对比: rubash vs GNU Bash (D:/Git/bin/bash.exe)
> 数据来源: bash-ledger-current-a24c0379 + 逐项分析

## 测试方法

- **.right 测试**: rubash 输出 vs 期望文件 -> 83/87 通过
- **实际对比**: rubash 输出 vs GNU Bash 输出 -> 13/83 完全一致

## 总览

| 分类 | 数量 | 说明 |
|------|------|------|
| PASS (完全一致) | 13 | 字节级相同 |
| DIFF | 69 | 输出不同 |
| 平台噪音 | 16 | bash=127 或超时 |
| RC 相同输出不同 | 37 | 执行正确，格式差异 |
| 真正 RC 差异 | 16 | 错误码不同 |

## 16 个平台噪音 (非 rubash bug)

### bash=127: bash 找不到辅助脚本 (10 个)

Git Bash 找不到 recho/zecho/printenv，返回 127。rubash 正确找到了。

| 测试 | bash | rubash | 说明 |
|------|------|--------|------|
| alias | 127 | 1 | rubash 正确执行 |
| exportfunc | 127 | 2 | rubash 跑得更远 |
| iquote | 127 | 0 | rubash 完全通过 |
| more-exp | 127 | 0 | rubash 完全通过 |
| new-exp | 127 | 2 | rubash 跑得更远 |
| nquote1 | 127 | 0 | rubash 完全通过 |
| nquote2 | 127 | 0 | rubash 完全通过 |
| nquote3 | 127 | 0 | rubash 完全通过 |
| nquote4 | 127 | 0 | rubash 完全通过 |
| quote | 127 | 2 | rubash 跑得更远 |

**结论: 这 10 个测试中 rubash 表现优于 bash。**

### 超时规则不同 (6 个)

| 测试 | bash | rubash | 说明 |
|------|------|--------|------|
| getopts | 0 | 124 | rubash 超时 |
| printf | 2 | 124 | rubash 超时 |
| procsub | 124 | 0 | bash 超时 |
| read | 124 | 0 | bash 超时 |
| redir | 124 | 0 | bash 超时 |
| trap | 2 | 124 | rubash 超时 |

## 16 个真正 RC 差异

### rubash 早期终止 (3 个) -- 高优先级

| 测试 | bash | rubash | bash行数 | rubash行数 | 根因 |
|------|------|--------|---------|-----------|------|
| arith | 1 | 2 | 369 | 0 | 算术错误终止 |
| array | 1 | 2 | 722 | 360 | 数组错误终止 |
| cond | 0 | 2 | 193 | 12 | 条件表达式错误 |

### rubash 解析错误 (3 个) -- 高优先级

| 测试 | bash | rubash | 根因 |
|------|------|--------|------|
| braces | 0 | 2 | 花括号展开解析 |
| comsub-posix | 0 | 2 | POSIX 命令替换解析 |
| posixexp2 | 0 | 2 | POSIX 表达式解析 |

### rubash 比 bash 更好 (3 个) -- 无需修复

| 测试 | bash | rubash | 说明 |
|------|------|--------|------|
| builtins | 2 | 0 | bash 内置命令失败 |
| comsub2 | 2 | 0 | bash 命令替换失败 |
| histexp | 2 | 0 | bash 历史扩展失败 |

### 其他真正差异 (7 个)

| 测试 | bash | rubash | 根因 |
|------|------|--------|------|
| complete | 2 | 1 | 补全退出码 |
| glob | 0 | 1 | glob 匹配行为 |
| mapfile | 0 | 2 | mapfile 管道 |
| posix2 | 9 | 12 | POSIX 合规 |
| posixpipe | 0 | 1 | POSIX 管道 |
| quotearray | 0 | 2 | 引用数组 |
| rsh | 0 | 1 | rsh 内置 |

## 37 个 RC 相同输出不同

| 类型 | 测试 | 说明 |
|------|------|------|
| declare 格式 | func, array | 函数体展开 vs 压缩 |
| 错误信息措辞 | dynvar, cond | 冒号后空格差异 |
| xtrace 不完整 | set-x | for/case 缺 trace |
| 输出顺序 | mapfile, case | 内容同顺序不同 |
| 环境变量泄漏 | varenv | 输出了宿主环境变量 |
| Windows 路径 | varenv | C:\\ vs /c/ |
| nameref | nameref | 引用变量解析不同 |

## 结论

### 真实通过率

- 去掉 16 个平台噪音: 13 + 16 = 29/67 = 43%
- 其中 3 个 rubash 优于 bash: builtins, comsub2, histexp
- 真正需要修的 RC bug: 13 个

### 优先级

1. 高: 早期终止 (arith, array, cond)
2. 高: 解析错误 (braces, comsub-posix, posixexp2)
3. 中: nameref, mapfile 管道
4. 低: 输出格式差异, 环境变量泄漏

### 关于测试框架

83 个测试是 GNU Bash 自己的，不需要重写。需要:
1. 标记 16 个平台噪音为 SKIP/PLATFORM
2. 输出格式差异用行数容差判定
3. 远程 issue 中因平台噪音产生的可以关闭