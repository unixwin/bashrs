# Rubash GNU Bash Semantic Migration Handoff

交给下一位 AI 的持续开发入口。目标是根据仓库内 pinned GNU Bash C 源码，严格闭合 Rubash 的 parser/lexer、parameter expansion、command substitution、arithmetic、printf、redirection、arrays、builtins 与 process execution 语义。

更新基线：2026-08-24。仓库状态以 git 和最新 raw artifacts 为准；旧 suite 数字只作背景。

## 1. 验收标准

每个修复必须具备 GNU source 对应、最小 GNU/Rubash probe、focused differential test、cargo check、git diff --check 和 durable attribution。不得修改 third_party/bash；不得用 .right runner 通过数宣称 actual parity；不得无界运行 full suite；不得只替换 text_lossy 或全局替换 sentinel；不得为了历史 expected output 改实现。

每轮开始和结束执行：

    git status --short --branch
    git diff --check
    ps -eo pid,comm,args | grep -Ei 'rubash|bash|cargo|suite' | grep -v grep

Windows/Winuxsh 是当前 release scope。Linux-only 差异不得驱动破坏 Windows 语义的修改。

## 2. 当前仓库状态

- Repository: D:/repo/rubash
- Branch: agentteams/typed-provenance
- HEAD: 1df9ec0a test: align readonly arithmetic case with bash
- Remote origin/agentteams/typed-provenance 已同步
- Worktree clean
- GNU executable: D:/Git/bin/bash.exe，GNU Bash 5.2.37

相关提交：

- 1df9ec0a：readonly arithmetic case 的 GNU/Rubash 一致行为，status 1、stdout 空、stderr 含 readonly variable。
- c236dcae：拒绝 array subscript 中递归 parameter expansion，避免栈溢出；保留合法的一层 nested subscript。
- 8291eceb：记录 raw read process-substitution gap。
- 3e6f081f：virtual fd stdin byte offset。
- 493c12cc：combined process-substitution streams regression。

## 3. 完成度口径

不要给项目一个没有定义的单一百分比：

- GNU Bash official actual-output ledger：13/83 exact，约 15.7%；70/83 有差异。差异含 Windows path/device、fixture、host boundary 和未归因项目，不等于 70 个 Rust bug。
- Windows CLI focused baseline：329/353，约 93.2%；只代表已覆盖 focused tests。
- bashdb focused slice：35 passed / 12 failed，约 74.5%；剩余集中在 source mapping、nested shell、breakpoint、variable、command runtime。
- semantic map：10 个高层域中 6 个 partial、1 个 scaffold、1 个 bridge、1 个 deferred、1 个 host-owned；核心域暂无可标为 real。

工程估计：核心语义迁移约 40%，只是工作量估计，不是验收指标。真实验收必须按 source family + focused differential closure 逐项完成。

权威顺序：最新 target/issue-suites/results raw output > 最新 issue-suite-diff-analysis attribution > semantic-ownership.tsv > bash-compat-issues.md 历史汇总。

## 4. GNU C 到 Rust owner

### Parameter / lexer

GNU：third_party/bash/subst.c 的 extract_dollar_brace_string、skipsubscript、skip_matched_pair、param_expand；parse.y 的 P_ARRAYSUB、P_DOLBRACE、parse_matched_pair；arrayfunc.c 的 array_variable_part、array_value_internal；variables.c。

Rust：src/lexer/word.rs、src/executor/parameter_core.rs、parameter_words.rs、parameter_errors.rs、embedded_parameters.rs、embedded_mutations.rs、arrays/executor.rs。

已完成：${a[${${i}}]} 在 GNU Bash 5.2.37 中是 bad substitution/status 1；Rubash 原先递归溢出。parameter_errors.rs 现在在 mutable whole-word expansion 前拒绝该递归形态，同时保留合法的一层 ${a[${i}]} 与 ${a[$i]}。提交 c236dcae。

### Command substitution / bytes

GNU：subst.c::read_comsub、param_expand；process_substitution.c。

Rust：substitution_metadata.rs、command_substitution.rs、command_substitution_values.rs、embedded_mutations.rs、read_io.rs、external_setup.rs、fd_table.rs。

已完成 typed capture/readback、NUL/trailing newline、invalid UTF-8 raw-byte marker、pipeline/timed/function/process-substitution 多条 regression。

关键 gap：SubstitutionOutput 在 assignment/legacy consumer 处仍通过 text_lossy 转 String；普通 scalar assignment 随后进入 env_vars: HashMap<String, String>。probe：x=$(printf '\\035'); printf '%s' "$x" | od -An -tx1。GNU 输出 1d，Rubash 当前为空。

正确迁移：whole command substitution assignment 保留 bytes；scalar variable storage/readback 使用 typed owner 或 side table；定义 overwrite、unset、function local、subshell clone、export 生命周期。不能只改 marker 或只改 text_lossy。

### Read/process substitution gap

probe：read x < <(printf '\\377\\n'); printf '%s' "$x" | od -An -tx1。GNU 输出 ff，Rubash 当前丢失 bytes。

当前链：fd_table byte input -> read_io.rs::read_virtual_fd_stdin -> String -> shell variable。需要 record-level byte carrier 贯穿 read 到变量写入边界；单独替换 String::from_utf8_lossy 不足。

### Arithmetic

GNU：expr.c、execute_cmd.c、subst.c。Rust：src/executor/arithmetic/、conditional.rs、parameter_core.rs。

readonly arithmetic case 实际语义是展开失败、中止当前命令、status 1、stdout 空、readonly diagnostic。测试名 readonly_arithmetic_case_pattern_aborts_without_mutating，提交 1df9ec0a。不要恢复旧 status 0/1.1 期望。

### printf

GNU：third_party/bash/builtins/printf.def 及 format/conversion helpers。Rust：src/builtins/printf/、src/executor/printf_path_builtins.rs 及 substitution consumers。

区分 format-string escape 与 %b argument escape。重点 probe：\\377、\\035、invalid UTF-8、NUL、%q、radix diagnostics。先确认 bytes 是否在 lexer/word expansion 已损失，再修改 printf owner。

### Redirection / fd / heredoc

GNU：redir.c、redir.h、execute_cmd.c、subst.c、process_substitution.c。Rust：fd_table.rs、redirection.rs、trap_exec.rs、read_io.rs、external_setup.rs、pipeline_exec.rs。

dynamic varredir 已用 GNU Bash 5.2.37 probe 对齐；failed replacement 保留旧 fd。下一 gate 是 ordered stderr/<> 三个 Rust failures 与 native vredir4/5/7/8 probes。

heredoc 有两个独立问题：上下文/状态收集，以及巨大输入性能/挂起；不要合并。

### Arrays / variables

GNU：array.c、array2.c、arrayfunc.c、assoc.c、variables.c。Rust：src/shell/variables.rs、src/shell/arrays/、src/executor/array_assignment_exec.rs、src/executor/arrays/。

VariableStore 已有 ShellValue::Scalar、IndexedArray、AssociativeArray，但大量 executor 仍用 String env mirror；当前是 partial/scaffold。剩余重点：delimiter encoding、compound assignment、sparse/negative index、nameref、empty array、subscript expansion。

## 5. Semantic map 摘要

权威文件：docs/semantic-ownership.tsv。

- redir/fd：partial；external child setup 和 env mirror 待迁移。
- subst/variables/typed values：partial；assignment/read/mapfile String boundary 未闭合。
- arrays/assoc/nameref：scaffold；需要替换 delimiter encoding。
- jobs/wait/fg/bg/disown：partial；Windows native stop/continue 与完整 builtin differential 待完成。
- set、umask：partial；选项、输出、错误码仍需覆盖。
- coproc/process substitution：bridge；endpoint tests 不等于移除 upstream bridge。
- readline/input：deferred/host-owned，不是当前 noninteractive gate。
- locale/intl：host-owned。
- dynamic braced names/getopts：partial；继续覆盖 nested names、empty sentinel、indirect assignment。

修改 map 后运行 scripts/validate-semantic-map.sh。

## 6. 推荐开发循环

1. 选已有 GNU source ownership 和最小 probe 的 root-cause gap。优先 raw read、scalar typed assignment、ordered redirection、arithmetic/printf context。
2. 用 Rust Command harness 或 fixture 固定 GNU/Rubash stdout bytes、stderr、status。复杂 array/escape 不要经过多层 shell wrapper。
3. 读取 GNU 调用方、错误路径、quote state、ownership 和 lifetime，不只 grep 函数名。
4. 只改 root owner；adapter 可保留，但明确何时允许 lossy conversion。
5. 每个 patch 至少运行：

    cargo check
    cargo test --test cli_tests <one-test> -- --exact --nocapture
    cargo test --lib <focused-module> -- --nocapture
    git diff --check

6. 加跑同一 source family 的 2-5 个相邻 regression，区分 timeout、Windows host difference、fixture mismatch 和 Rust bug。
7. 只提交具体文件并推送 origin/agentteams/typed-provenance；最终报告 source mapping、tests、cargo check、commit、push、remaining gap。

## 7. 已知失败实验

- 把 quoted-parameter C0 marker x1d 全局换成 PUA：破坏 POSIX quoted brace、quoted assignment、parameter replacement；已回滚。
- 只替换 assignment text_lossy：bytes 已进入 String/env map，无法恢复 1d provenance。
- 只修 mapfile/raw read producer：变量 materialization 仍会丢 bytes。
- 凭直觉关闭 dynamic varredir old fd：GNU 5.2 failed replacement 保留旧 fd。
- 不要同时拒绝一层 nested array subscript parameter 与二层递归 parameter；前者合法，后者才是已验证 bad substitution。
- 用 .right 87/87 宣称 actual parity：只是 checked-in expected files。
- 用 shell wrapper 传递复杂 array/escape probe：曾导致 GNU 与 Rubash 同时收到 malformed command。

## 8. 下一位 AI 第一轮

1. 读取本文件和四份入口文档：gnu-bash-compatibility-implementation-plan.md、issue-suite-diff-analysis.md、bash-compat-issues.md、bash-source-map.md。
2. 检查 git status、process hygiene、最新 raw result。
3. 闭合 read raw-byte record 到 scalar variable，或 scalar assignment typed carrier；必须有 bytes-level regression 才能提交。
4. 若跨越多个 owner boundary，拆成连续小提交；不要一次迁移所有 String consumers。
5. 每完成一个 source family，更新 semantic map 和 dated attribution，再进入下一族。

持续执行 source -> probe -> owner -> focused test -> docs -> commit/push，直到 core rows 有可验证的 real owner，或明确记录 Windows host/deferred boundary。
