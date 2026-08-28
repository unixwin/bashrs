# Rubash 测试策略

## 三层测试体系

### 第一层: .right 兼容性测试 (87 个)

- 工具: scripts/run-bash-upstream-tests.sh
- 对比: rubash 输出 vs .right 期望文件
- 通过率: 83/87 = 95.4%
- 意义: rubash 输出是否正确

### 第二层: 实际输出对比 (83 个)

- 工具: scripts/run-bash-actual-ledger.sh
- 对比: rubash 输出 vs GNU Bash 输出
- 结果: 13 完全一致, 37 接近, 16 平台噪音, 16 真 bug
- 意义: rubash 和 bash 行为差异

### 第三层: rubash 专用回归测试 (建议新建)

- 针对 13 个真正的 RC bug
- 不受平台噪音干扰
- 可以在 CI 中快速运行

## 83 测试根因分类 (详见 docs/bash-83-test-root-cause-analysis.md)

| 分类 | 数量 | 处理方式 |
|------|------|----------|
| 完全一致 | 13 | 无需处理 |
| 平台噪音 | 16 | 标记 SKIP, 不计入 bug |
| RC 相同输出不同 | 37 | 部分是格式差异, 部分是真 bug |
| 真正 RC 差异 | 16 | 其中 3 个 rubash 优于 bash |
| 需要修复的 bug | 13 | 重点修复 |

## 13 个需要修复的 bug

### 早期终止 (3 个)
- arith: 算术错误终止脚本
- array: 数组操作错误终止
- cond: 条件表达式错误终止

### 解析错误 (3 个)
- braces: 花括号展开解析
- comsub-posix: POSIX 命令替换解析
- posixexp2: POSIX 表达式解析

### 其他 (7 个)
- complete, glob, mapfile, posix2, posixpipe, quotearray, rsh

## 避免教条主义的规则

1. **不盲目追求 83/83**: 平台噪音不算 bug
2. **区分 rubash 优于 bash 的情况**: builtins/comsub2/histexp
3. **用行数容差判定输出差异**: ≤5行差异算 PASS
4. **优先修高影响的 bug**: 早期终止 > 解析错误 > 格式差异
5. **远程 issue 要核实**: 不是所有 DIFF 都是 rubash 的问题