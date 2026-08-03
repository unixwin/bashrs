# rubash 全量达标计划（2026-08-03 落盘）

> 目标：通过全部测试套件（差分 26 + bash 官方 83 + 上游 87 + oil 684 + mksh 436 + ksh93 46 + busybox 143），解决远程全部 7 个 issue（#20-#26）。
> 原则：P0 挂起/DoS 优先 → 最短路径消除 FAIL → 大套件按根因族推进 → 每阶段跑全套回归（差分 + winuxsh suites + 上游 + Rust）。
> 当前基线（2026-08-03 会话结束）：**差分 26/26 全 PASS**（阶段 1 完成）；bash 官方 83 tests 14/83（阶段 2 进行中）；上游 86/87（run-minimal 归入路径转换家族）；Rust 69/70。
> 会话交接：`memory/handoff-next-session.md`（新会话先读这个 + 本文件 + `memory/compat-fixes-20260803.md`）。

## 阶段 0：回归测试环境修复（阻塞所有验证）
- 上游测试 `tests/bash` 缺失（cp 失败、`usr/bin/bash.exe` 缺失）：修复 runner 复制逻辑/环境（scripts/run-bash-upstream-tests.sh），恢复 87/87 基线
- winuxsh suites 驱动确认可用（WINUXSH_RUNNER=rubash 已验证 bash-tests 14/83）

## 阶段 1：差分 26 case 全 PASS（最短路径）
- case-01（for+heredoc 词法组合）：heredoc body 收集与逻辑行交互——词法层修复（heredoc 未闭合跨行收集）
- case-03（嵌套命令替换引号残留）：`$(printf '` 被执行链预处理吞——路径选择修复（expand_command_substitution_mut 特例优先已部分）
- case-05（命令替换内 tilde）：split_shell_words 剥引号——新增 split_shell_words_with_quote_info 或引号保留方案
- case-10（版本身份）：BASH_VERSINFO 对齐 bash 版本号（或标记为预期保留，接受 1 个 FAIL）

## 阶段 2：bash 官方 83 tests（14 → 83）
- 37 项可靠差异 + 32 项待归因（#25）——按根因族归类（与差分同批）
- 每项对照 bash 实际输出（winuxsh bash-tests-difftest.sh）逐项修复

## 阶段 3：P0 挂起/DoS
- heredoc_huge（#26）：外部命令管道并发（os_pipe crate 或父进程流式转发）——`yes|head|md5sum` 不挂起
- coproc 挂起（#21 §2.1）：COPROC 数组 fd 映射 + `<&N` 从 coproc 管道读

## 阶段 4：内置族整体（#24 约 120 项）
- umask（18 项符号模式）、trap（37 项 ERR/-p/-l 语义）、kill（15 项 -l/-s）、set/shopt（35 项）、echo、cd、jobs（17 项）
- 对照 bash builtins/*.c + *.def（docs/bash-source-map.md）

## 阶段 5：oil 684 + mksh 436（大套件，按根因族）
- 同批根因族（case/heredoc/eglob/expand/error-code）——与差分/上游重叠修复
- mksh 特有：`n#base` 进制（已验证通过）、$LINENO-[[ ]]（已验证通过）

## 阶段 6：busybox 143 + ksh93 46
- busybox：heredoc_huge（阶段 3）、vars 29、redir 14、signals 11、解析、psubst
- ksh93：bash 支持子集（ksh 特有语法如复合变量非目标，忽略）

## 阶段 7：回归固化 + 差距评估 + 发布
- 每阶段聚焦用例固化到 tests/difftest/cases（case-NN）
- 更新 docs/bash-compat-issues.md 差距评估（DIFF 单调下降）
- winuxsh tag 发布协调（rubash 合并主分支 → winuxsh 用最新 rubash 跑差分）

## 执行顺序说明
- 阶段 0/1 是**当前最近目标**（差分 4 FAIL 可直接攻）
- 阶段 2/3 并行推进（bash 83 与 P0 挂起）
- 阶段 4-6 是系统性大项（内置族、oil/mksh/busybox），按根因族逐项
- 每阶段完成即跑全套回归并 push master
