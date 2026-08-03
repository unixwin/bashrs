# rubash 兼容性差距总览（issue #20-#26 汇总与维修计划）

> 汇总日期：2026-08-03（数据修正版：同步各 ISSUE 评论的最终数字）。数据来源：unixwin/rubash issue #20-#26 + 本地差分/上游测试。
> 核心结论：**7 个 issue 的差异高度重叠**——同一批根因族在不同套件（oil spec / mksh / ksh93 / bash 官方 / busybox）中反复命中。按根因族维修，而非按 issue 逐个修。

## 一、7 个 issue 概览（最终数字，与 ISSUE 评论一致）

| issue | 套件 | 规模 | 核心领域 |
|-------|------|------|----------|
| #20 | 自研 probe | 20+ 项 | 命令替换状态污染 / heredoc / case / 参数替换 / 路径转换 / 重定向 / 调试钩子 |
| #21 | 自研 probe r2 | 11 项 | 花括号 / IFS 分词 / glob 反斜杠 / BASHPID / 进程替换 / **coproc 挂起** / wait / 特殊内建管道 |
| #22 | oil spec（首跑半程快照） | 219 项（当时快照） | alias 语义 / **语法宽松度** / 数组赋值族 / 算术错误码 / jobs-dirs |
| #23 | oil 全量 + mksh 全量 + ksh93 全量 | **684 / 436 / 46** | case / heredoc / eglob / expand / break 越界 / 错误码 / ksh 复合变量 |
| #24 | oil 684 + mksh 436（全量清单聚焦） | 高频领域 | **内置族整体**（umask/trap/kill/set/echo/cd/shopt）/ word-split 35 / var-op 48 / nameref / mksh 进制 |
| #25 | bash 官方 83 tests | 14/83 一致，37 可靠差异（另 32 项待归因） | 与 #20/#21/#22 同批根因族（.right 旧期望掩盖） |
| #26 | busybox ash_test | **143 项**（17/17 目录完成） | **heredoc_huge 挂起（P0 DoS）** / vars 29 / redir 14 / signals 11 / 解析 / psubst |

> 差距总量：**1,345+ 项**（31 手动 + 684 + 436 + 37 可靠 + 46 + 143），全部已归入上述 ISSUE。

## 二、根因族聚类（跨 issue 交叉验证）

### 族 A：heredoc（P0 — 挂起/DoS，两个触发面）
- **触发面 1（上下文/状态）**：#20 §2.1 接收者上下文展开、#23/#25 heredoc 边界；本地已定位：heredoc body 词法收集在特定前置状态失败 + 后台 stdin 挂起（见 memory）
- **触发面 2（大输入/性能）**：#26 `heredoc_huge.tests`——**巨大 heredoc 输入** bash 秒完成、winuxsh 20s/30s 超时被杀（rc=124）、残留 .tmp——疑似缓冲/死循环/性能问题，与触发面 1 相互独立，需分别验证
- 修复点：heredoc.rs `<<` vs `<<<` 区分、body 收集状态复位、跨行收集、大输入缓冲路径

### 族 B：命令替换状态污染（最难，间歇性）
- #20 §1：深层函数链赋值捕获丢失 / cd PWD 拼接（已修 #12）/ printf 损坏 / 函数参数丢失
- 本地：#12 已修（PWD 拼接）；其余为执行后状态污染，需 debug 工具定位
- **已实测的机制之一：stdin 消费泄漏**——busybox 差分驱动中，测试脚本内的 `read` 会消费 while 循环的输入流（子进程继承 stdin fd），需显式 `< /dev/null` 才正常；推测与文档猜测的 FUNCTION_STDIN 环境变量泄漏同源，可作为定位线索
- 修复点：命令替换执行器状态隔离、FUNCTION_STDIN 等环境变量泄漏、子进程 stdin fd 隔离

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
- 已修部分：DEBUG/RETURN trap、PS4、BASH_COMMAND（对应 PR #3/#4 —— **需核实是否已合入**，git log 近期提交未见调试钩子修复）
- **未修**：FUNCNAME 缺 main（差分 case-10，rubash#20 §10.5）、BASH_VERSINFO、trap DEBUG/RETURN/EXIT 触发语义、#24 trap 37 项中的其余部分

### 排除项：ksh 特有语法（非兼容目标，勿修）
- ksh93 复合变量/多维数组（`${a[0][0]}`、`${p.len}`）：**bash 本身不支持**，rubash 无需兼容——ksh93 测试中这类用例的差异**不是缺口**，直接忽略
- **注意区分**：ksh93/mksh 测试里 bash 也支持的子集才是真差异，需逐个确认：
  - `n#base` 进制字面量（`$((16#ff))`，mksh 29 项中 bash 支持的部分）——bash 支持，rubash 需对齐
  - `$LINENO` 在 `[[ ]]` 内的展开（ksh93/mksh 互证）——bash 支持，需对齐
  - alias/case/heredoc/quoting 等 POSIX/bash 通用语义——bash 支持，是真差异（已归入对应族）

## 三、维修顺序建议（按根因族，非按 issue）

原则：**P0 DoS 优先 → 高影响确定性差异 → 单点收益大的内置族 → 深层机制**。
每族修完**立即跑全套回归**（差分 23 + 上游 87 + 该族聚焦用例），不攒到最后。

| 优先级 | 根因族 | 依据 | 预计工作量 |
|--------|--------|------|-----------|
| P0 | 族 A heredoc 挂起（#26 大输入 + #23/#25 边界） | DoS + 常用特性；本地已定位触发面 1 | 中（词法层，有定位结论） |
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
| P3 | 族 L 调试钩子残余（FUNCNAME/BASH_VERSINFO/trap 语义） | 已修核心，残余少 | 小 |

## 四、测试与验证策略

1. **回归基线**：差分测试（tests/difftest，23 case，当前 19 PASS）+ 上游测试（87/87）+ Rust 测试
2. **每族聚焦用例**：从 winuxsh probe/suites（oil/mksh/busybox）提取该族代表用例，固化到 tests/difftest/cases/ 作为回归
3. **大型脚本**：自建综合脚本（已发现 heredoc 挂起）+ 真实构建脚本（如 Git contrib、经典 configure 脚本），bash vs rubash 对比
4. **GNU Bash 源码映射**：`docs/bash-source-map.md` 有 .c/.def → .rs 映射；修每族时先读对应 C 源码（subst.c/execute_cmd.c/builtins/*.c）再改 .rs，避免盲改
5. **winuxsh 侧套件**：`winuxsh/scripts/probe/suites/` 下 5 个差分驱动（spec/mksh/ksh93/bash-tests/busybox）可随时全量回归，DIFF 数必须单调下降

## 五、GNU Bash 差距评估（当前，最终数字）

- 上游 run-* 套件（.right 对比）：87/87 —— **但 .right 是旧期望，掩盖实际差距**
- bash 官方 83 tests 实际输出对比：14/83（#25；另 32 项 bash 侧 rc 非零，**待用 run-bash-upstream 完整环境复核归因**）
- 差分测试 26 case：22/26（真 bug 3 个：case-01/03/05 + 版本身份 1 个：case-10；新增 case-24 var-op / case-25 arith / case-26 alias 全 PASS，2026-08-03）
- oil spec：228 文件 684 项差异（#23/#24）——**系统性差距仍在词法/解析/执行边界**
- mksh：436 项；ksh93：46 项（**其中 ksh 特有语法部分为非目标**，bash 支持子集需逐个筛选）；busybox ash：143 项（含 vars/signals 新领域）
- **验收量化门槛（引用 winuxsh#48）**：bash 官方 83/83、oil 684→0、mksh 436→0、ksh93 错误数→0、手动 31 项复测、cloc rubash/src ≥80%（当前 60,708 / bash 132,879 ≈ 45.7%）
- 结论：距完整 GNU Bash 支持仍有**中-大规模差距**，集中在：heredoc 机制（含大输入挂起）、命令替换状态隔离、IFS 分词边界、语法错误处理、内置族完整语义（ksh 特有语法不在目标内）
