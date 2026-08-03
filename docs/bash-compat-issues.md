# rubash 兼容性差距总览（issue #20-#26 汇总与维修计划）

> 汇总日期：2026-08-03。数据来源：unixwin/rubash issue #20-#26 + 本地差分/上游测试。
> 核心结论：**7 个 issue 的差异高度重叠**——同一批根因族在不同套件（oil spec / mksh / ksh93 / bash 官方 / busybox）中反复命中。按根因族维修，而非按 issue 逐个修。

## 一、7 个 issue 概览

| issue | 套件 | 规模 | 核心领域 |
|-------|------|------|----------|
| #20 | 自研 probe | 20+ 项 | 命令替换状态污染 / heredoc / case / 参数替换 / 路径转换 / 重定向 / 调试钩子 |
| #21 | 自研 probe r2 | 11 项 | 花括号 / IFS 分词 / glob 反斜杠 / BASHPID / 进程替换 / **coproc 挂起** / wait / 特殊内建管道 |
| #22 | oil spec（1/2 套件） | 219+ 项 | alias 语义 / **语法宽松度** / 数组赋值族 / 算术错误码 / jobs-dirs |
| #23 | oil 全量 + mksh + ksh93 | 427+ / 153+ / 若干 | case / heredoc / eglob / expand / break 越界 / 错误码 |
| #24 | oil 684 + mksh 436 | 全量清单 | **内置族整体**（umask/trap/kill/set/echo/cd/shopt）/ word-split 35 / var-op 48 / nameref / mksh 进制 |
| #25 | bash 官方 83 tests | 14/83 一致，37 可靠差异 | 与 #20/#21/#22 同批根因族（.right 旧期望掩盖） |
| #26 | busybox ash_test | 69+ 项 | **heredoc_huge 挂起（P0 DoS）** / 解析 / psubst / glob |

## 二、根因族聚类（跨 issue 交叉验证）

### 族 A：heredoc（P0 — 挂起/DoS）
- #26 heredoc_huge **永久挂起**（rc=124，P0 DoS）；#20 §2.1 接收者上下文；#23/#25 heredoc 边界
- 本地已定位：heredoc body 词法收集在特定前置状态失败 + 后台 stdin 挂起（见 memory）
- 修复点：heredoc.rs `<<` vs `<<<` 区分、body 收集状态复位、跨行收集

### 族 B：命令替换状态污染（最难，间歇性）
- #20 §1：深层函数链赋值捕获丢失 / cd PWD 拼接（已修 #12）/ printf 损坏 / 函数参数丢失
- 本地：#12 已修（PWD 拼接）；其余为执行后状态污染，需 debug 工具定位
- 修复点：命令替换执行器状态隔离、FUNCTION_STDIN 等环境变量泄漏

### 族 C：IFS 分词 / word-split（高影响）
- #21 §1.2 空字段、§1.3 `$*` IFS（已修 #13）；#24 word-split 35 项大规模扩展
- 修复点：IFS 分词器边界（空字段/多字符 IFS/引号组合）

### 族 D：语法宽松度（bash 报错 rc=2、rubash 静默 rc=0）
- #22 §B、#23 eglob/bksl-nl、#24 echo typed args、#26 ash-parsing
- 修复点：解析器对 bash 拒绝的语法报错（数组 `a= (1 2)`、`[[ ) ]]`、extglob 错误等）

### 族 E：内置族整体异常（#24，覆盖约 120 项，单点修复收益大）
- umask（18 项，符号模式解析）、trap（37 项，ERR 语义/列表/-p/-l）、kill（15 项，-l/-s）、set/shopt（35 项）、echo、cd、jobs（17 项）
- 修复点：各内置的完整选项/输出/错误码对齐（可对照 bash builtins/*.c + *.def）

### 族 F：数组赋值族（#22/#23，约 60 项）
- `+=`（`s+=(...)`）、稀疏负索引、`${@:off:len}`、declare 空格、空数组插值
- 修复点：数组赋值解析与插值边界

### 族 G：算术错误码（#22/#23/#24，约 25 项）
- bash 报错 rc=1（`'1'` 常量/浮点/负指数/nounset 算术）、rubash rc=0 静默
- 修复点：算术求值错误传播

### 族 H：alias 语义（#22/#23，约 15 项）
- 单引号 alias 也被展开（安全防御失效）、管道中 alias 失效（rc=127）、`unalias -a`、无参列出
- 修复点：alias 展开的引用检查 + 管道上下文

### 族 I：参数替换边界（#20 §2.5、#24 var-op 48、#26 psubst）
- `${v=}`/`${v:-}`/`${v:?}` 组合、slice 负偏移、patsub 反斜杠（#12 部分已修）
- 修复点：var-op 组合边界（对照 subst.c）

### 族 J：路径/glob（#21 §1.4 glob 反斜杠、#20 §4）
- glob 结果含反斜杠 → `${f##*/}` 失效（#12 已修 PWD/替换部分）
- 修复点：glob 结果路径分隔符归一化

### 族 K：coproc/进程替换/后台挂起（P0）
- #21 §2.1 coproc 挂起、进程替换嵌套输出丢失（§1.5）、#26 挂起家族
- 修复点：coproc/进程替换执行器（与族 A 的挂起模式同源）

### 族 L：调试钩子（#20 §6、#25 dbg-support）
- 已修（#3/#4：DEBUG/RETURN trap、PS4、BASH_COMMAND）；#24 trap 37 项中的部分

## 三、维修顺序建议（按根因族，非按 issue）

原则：**P0 DoS 优先 → 高影响确定性差异 → 单点收益大的内置族 → 深层机制**。
每族修完**立即跑全套回归**（差分 23 + 上游 87 + 该族聚焦用例），不攒到最后。

| 优先级 | 根因族 | 依据 | 预计工作量 |
|--------|--------|------|-----------|
| P0 | 族 A heredoc 挂起（#26 P0 + #23/#25） | DoS + 常用特性；本地已定位 | 中（词法层，有定位结论） |
| P0 | 族 K coproc/进程替换挂起（#21 §2.1） | DoS | 中 |
| P1 | 族 D 语法宽松度（#22/#23/#24/#26） | 影响所有脚本解析正确性 | 中（解析器报错） |
| P1 | 族 C IFS 分词（#24 35 项） | 高影响、确定性 | 小-中（#13 已修核心） |
| P1 | 族 E 内置族（#24 约 120 项） | 单点修复、覆盖广 | 中（逐内置对齐） |
| P2 | 族 H alias（#22/#23） | 15 项、安全防御 | 小 |
| P2 | 族 F 数组（#22/#23 60 项） | 中等 | 中 |
| P2 | 族 G 算术错误码（25 项） | 中等 | 小 |
| P2 | 族 I 参数替换边界（#24 48 项） | 中等 | 中 |
| P2 | 族 J glob 路径（#21 §1.4） | 已部分修 | 小 |
| P3 | 族 B 状态污染（#20 §1） | 最难、间歇性 | 大（需 debug 工具） |
| P3 | 族 L 调试钩子残余（#24 trap 部分） | 已修核心 | 小 |

## 四、测试与验证策略

1. **回归基线**：差分测试（tests/difftest，23 case，当前 19 PASS）+ 上游测试（87/87）+ Rust 测试
2. **每族聚焦用例**：从 winuxsh probe/suites（oil/mksh/busybox）提取该族代表用例，固化到 tests/difftest/cases/ 作为回归
3. **大型脚本**：自建综合脚本（已发现 heredoc 挂起）+ 真实构建脚本（如 Git contrib、经典 configure 脚本），bash vs rubash 对比
4. **GNU Bash 源码映射**：`docs/bash-source-map.md` 有 .c/.def → .rs 映射；修每族时先读对应 C 源码（subst.c/execute_cmd.c/builtins/*.c）再改 .rs，避免盲改

## 五、GNU Bash 差距评估（当前）

- 上游 run-* 套件（.right 对比）：87/87 —— **但 .right 是旧期望，掩盖实际差距**
- bash 官方 83 tests 实际输出对比：14/83（#25）
- 差分测试 23 case：19/23（真 bug 3 个 + 版本身份 1 个）
- oil spec：228 文件 684 项差异（#24）——**系统性差距仍在词法/解析/执行边界**
- 结论：距完整 GNU Bash 支持仍有**中-大规模差距**，集中在：heredoc 机制、命令替换状态隔离、IFS 分词边界、语法错误处理、内置族完整语义
