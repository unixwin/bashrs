# Rubash GNU Bash Semantic Migration Handoff

交给下一位 AI 的持续开发入口。目标是根据仓库内 pinned GNU Bash C 源码，严格闭合 Rubash 的 parser/lexer、parameter expansion、command substitution、arithmetic、printf、redirection、arrays、builtins 与 process execution 语义。

更新基线：2026-08-24（第二版，重写自同日第一版）。仓库状态以 git 和最新 raw artifacts 为准；旧 suite 数字只作背景。

## 1. 验收标准（不变，约束性）

每个修复必须具备 GNU source 对应、最小 GNU/Rubash probe、focused differential test、cargo check、git diff --check 和 durable attribution。不得修改 third_party/bash；不得用 .right runner 通过数宣称 actual parity；不得无界运行 full suite；不得只替换 text_lossy 或全局替换 sentinel；不得为了历史 expected output 改实现——陈旧测试必须先用实时 GNU 探针重推证据再改写。

每轮开始和结束执行：

    git status --short --branch
    git diff --check
    进程卫生：检查并清理卡死的 rubash.exe / bash.exe / cargo / 测试进程

Windows/Winuxsh 是当前 release scope。Linux-only 差异不得驱动破坏 Windows 语义的修改。

## 2. 当前仓库状态

- Repository: D:/repo/rubash
- Branch: agentteams/typed-provenance
- HEAD: 1790a6e4 fix: preserve raw bytes for external fd stdin
- Remote origin/agentteams/typed-provenance 已同步
- Worktree clean except the pre-existing untracked `max_line` diagnostic artifact
- GNU executable: D:/Git/bin/bash.exe，GNU Bash 5.2.37（差分基准）

本会话累计提交（全部已推送）：

- 08c2aac3 fix: carry raw bytes through file-backed read inputs
- 6ab4497d fix: carry raw bytes through pipeline stage transport
- 52725a17 fix(arith): word-expansion failures abort the current list per GNU evidence
- e23c5af9 fix(arith): brace-group frames absorb word-expansion aborts
- 226a706d fix(arith): word failures in if conditions abandon the whole compound
- 18a88b75 docs: record frame-absorption wins and defer per-line list boundary
- cb39127a fix: decode raw markers at echo output boundary
- 3e3e3142 fix: preserve raw bytes through coproc read
- a89d5c11 fix: protect C0 bytes in scalar assignments
- 5d6d8e64 fix: preserve C0 bytes in backtick assignments
- 1790a6e4 fix: preserve raw bytes for external fd stdin

工具链事实（本会话实测）：增量构建后二进制可能滞后于源码；行为矩阵前强制清除 target/debug/.fingerprint 后 cargo build 并核对行为。多轮字符串拼接编辑 CRLF 文件曾造成重复块/未闭合括号——对复杂锚点先 dump 精确字节再单次替换。

## 3. 完成度口径

不要给项目一个没有定义的单一百分比：

- GNU Bash official actual-output ledger：13/83 exact（旧口径，仅背景）。
- Windows CLI focused baseline：本次实测全量 cli_tests 为 363 passed / 12 failed；12 个失败全部是已知 bashdb 预存族（source mapping、nested shell、breakpoint、variable runtime），与算术/raw-byte 工作无关（stash 对照验证过）。
- 算术兼容家族（cli_tests arith 过滤）：33/33 绿。
- 原始字节回归切片（executor_tests part_047）：38/38 绿，包含 direct echo marker boundary。
- lib 单测：277/277 绿。
- semantic map：权威文件 docs/semantic-ownership.tsv，改后必跑 scripts/validate-semantic-map.sh。

工程估计保持保守：核心语义迁移约四成强，验收仍按 source family + focused differential closure 逐项闭合。

权威顺序：最新 target/issue-suites/results raw output > 最新 issue-suite-diff-analysis attribution > semantic-ownership.tsv > bash-compat-issues.md 历史汇总。

## 4. 本会话已闭合的 source family

### Raw byte 载体（读路径）

probe 家族 target/probe-rawbytes/{p*,d*,u*,t*}。五个 fs::read_to_string 输入点（exec N<file、动态 {fd}<file、临时输入重定向等）收敛到 substitution_metadata.rs::read_shell_input_file；所有管线传输接缝改为 bytes_to_shell_text 生产 / shell_text_to_raw_bytes 消费。新增反函数 shell_text_to_raw_bytes 及 roundtrip 单测。

echo 写入路径已闭合：`builtins/echo.def` 对应的 `src/builtins/echo.rs::write_echo_decoded` 在最终 sink 消费 RAW_BYTE_MARKER，`src/executor/shift_echo_builtins.rs` 的各重定向分支均已接入；GNU/Rubash probe 均为 61 ff 62。提交 `cb39127a`。注意 printf 已有独立 raw-byte boundary，勿二次解码。

### Arithmetic word-expansion fatality（本期主线）

GNU 5.2.37 实证（probes a1/b1/b3/b5/e5/g0..g3/s1..s3/x1..x3/z1/z2/y1..y3/f3/f4/d1..d3）：任何分类的词展开算术错误弃当前命令列表余项。实现：

- 新变体 ExecuteError::ExpansionFailure(i32)，与 exit/errexit 的 ExitCode 区分：
  - 词展开致命构造点：command_execute.rs 与 command_prepare.rs。
  - 函数帧吸收为提前返回（f3：调用者见 end-sub:1 继续）；exit N 穿透函数帧（x2 rc=3 不变）。
  - 大括号组帧吸收组尾（y1/y3），pipeline_exec.rs::execute_brace_group_pipeline。
  - if 条件上下文经 inside_compound_condition 门控让失败穿透函数帧、整体弃置复合命令（f4）。
  - 子壳边界、$() 状态助手、管线阶段捕获处把 ExpansionFailure 映射为捕获状态。
- 谓词 arithmetic_expansion_is_fatal = arithmetic_error_category(expression).is_some()；expr.c 分类补齐两个诊断分支（logical_rhs_assignment_token / empty_ternary_branch_token），文本与 GNU 字节一致：attempted assignment to non-variable (error token is "=42") 与 expression expected (error token is ": ")。
- 四个旧 continue 断言测试按实证重写；新增函数帧包含测试。

grand 矩阵现状：24 形 19 平。z1/z2 见第 5 节延迟项；逐形差异细节在 docs/issue-suite-diff-analysis.md 两则 2026-08-24 条目。

## 5. 仍然开放的 owner 边界（按优先级）

1. coproc/read raw-byte 边界已闭合：对照 GNU `builtins/read.def`，`src/executor/read_io.rs` 的 coproc `read -u` 记录现在通过 `bytes_to_shell_text` 保留 RAW_BYTE_MARKER；GNU/Rubash 均输出 `ff`，新增 CLI differential test 通过，bounded `c_command_` slice 61/61。提交 `3e3e3142`。
2. scalar assignment typed carrier 仍是开放项，但首个 C0 collision 子边界已闭合：whole `$(...)` assignment 使用 assignment-specific marker materialization，已覆盖 `0xff`/`0x1d`、local、subshell 和 export。剩余 mixed/复合 backtick、overwrite/unset 交互及最终 typed store owner 仍需继续迁移。
3. z1/z2 每物理行列表边界：GNU 规则=顶层词错只终止所在行的命令列表，后续行照跑。首次尝试（文件脚本走 stdin drive_command_stream 增量喂入器）使两形状平价但挂死/回归 examples::* 与 fd_redirects::c_external_* 共 11 个夹具，已回退。重试前提：设计真正的命令边界读取器（含 heredoc 收集与续行门控），先针对挂死夹具族做饥饿探针定位。
4. ordered stderr 与 native vredir4/5/7/8 probes（redir 家族，未动）。
5. external child cwd/path 已由 fresh GNU/Rubash probe 证明一致；persistent-fd raw-byte transport 已闭合（1790a6e4）。剩余 inherited fd/env mirror 边界仍需区分 Windows host 行为和 Rubash-owned setup。
6. bashdb 12 个预存失败（source mapping/nested shell/breakpoint 族），每个应驱动一次 root-cause 修复而非 bashdb 补丁。

已闭合：echo 写入路径字节统一（GNU `builtins/echo.def`；提交 `cb39127a`）。`write_echo_decoded` 在最终 echo sink 消费 RAW_BYTE_MARKER，同时保留 `echo -e` 已产生的真实 raw bytes；focused echo 9/9、`command_chaining::part_047` 38/38、GNU/Rubash `61 ff 62` probe 均通过。

## 6. 已知失败实验（继承+新增）

继承第一版全部条目（C0 全局换 PUA、只换 assignment text_lossy、只修 producer、凭直觉关 dynamic varredir old fd、nested subscript 二分误判、.right 宣称 parity、wrapper 传复杂 probe）。

新增：

- 凭旧 expected output 维持继续断言：arith continue 测试被实时 GNU 全数证伪（1=2 / 1++ / 1/0 / 08 在顶层全部中止 rc=1）。
- 未经实证的帧拓扑大重构：清单级 depth 计数 + 外层吸收模型一次性铺开产生多处回归；正确粒度是每帧一个吸收点、每个 commit 一帧。引入 ExpansionFailure 时同步清掉遗留 script_mode_nonfatal 通道，否则两条真相源极难排查。
- 增量喂入器整体替换文件解析：z1/z2 成立但 11 夹具挂死回退（细节见第 5 节第 2 条）。
- Cargo 指纹缓存竞态：编辑已落盘但 cargo 判 fresh，行为疑似未生效；指纹清除 + 二进制内 grep 标记串是可靠仲裁。

## 7. 下一位 AI 第一轮

1. 读取本文件和四份入口文档：gnu-bash-compatibility-implementation-plan.md、issue-suite-diff-analysis.md、bash-compat-issues.md、bash-source-map.md。
2. git status / 进程卫生 / 最新 raw result 三查。
3. 下一切片：处理 scalar command-substitution assignment carrier，覆盖 overwrite、unset、function local、subshell clone 和 export。
4. 继续 scalar assignment 的 mixed/复合 backtick/overwrite/unset 边界；随后处理 ordered stderr/vredir primitive，并保持每个 GNU source family 一个 focused commit。
5. z1/z2 物理行边界、ordered stderr/vredir、external child cwd/path、bashdb 失败和 arrays 保持在开放清单中。
6. 若跨越多个 owner boundary，拆成连续小提交；每个 source family 都必须更新 semantic map 和 dated attribution。

持续执行 source -> probe -> owner -> focused test -> docs -> commit/push，直到 core rows 有可验证的 real owner，或明确记录 Windows host/deferred boundary。
