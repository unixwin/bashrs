# 会话交接（2026-08-03）→ 新会话从这里继续

> 本文件是**会话交接文档**：新会话打开后先读本文件 + `docs/bash-mastery-plan.md`（阶段计划）+ `memory/compat-fixes-20260803.md`（根因记录）。
> 目标（docs/bash-mastery-plan.md）：通过全部套件（差分 26 + bash 官方 83 + 上游 87 + oil 684 + mksh 436 + ksh93 46 + busybox 143），解决远程全部 7 个 issue（#20-#26）。

## 一、当前精确状态（2026-08-03 会话结束时）

| 套件 | 状态 | 说明 |
|---|---|---|
| 差分 26 case | **26/26 全 PASS** ✅ | 阶段 1 完成（case-01/03/05/10 全部修复） |
| 上游 87 | **86/87** | 阶段 0 完成；run-minimal 的 `/usr/bin/rm` exec 是 Windows 路径转换问题 |
| bash 官方 83 | **14/83**（69 DIFF） | 阶段 2 进行中——下一轮主攻 |
| Rust 测试 | 69/70 | script_sort_pos_params 基线失败（路径相关） |
| oil/mksh/ksh93/busybox | 未跑（阶段 5/6） | 大套件，按根因族推进 |

## 二、下一步的确切第一步（阶段 2：bash 官方 83 tests）

69 DIFF 已分类（见 `../winuxsh/scripts/probe/suites/bash-tests-diffs.txt`）：
1. **先修非挂起族**（不依赖并发管道）：
   - **rc=127**（命令找不到）：exportfunc/iquote/more-exp/nameref/new-exp/nquote1-4/quote——测试里的命令 rubash 找不到（PATH/内置缺失）
   - **rc=2**（语法错误，族 D）：arith-for/builtins/complete/comsub2/errors/glob-bracket/histexp/parser/posixexp——bash 报错 rubash 静默
   - **rc=0**（stdout 差异）：各类语义差异——逐个对照 bash 实际输出修
2. **挂起族最后**（6 个 rc=124：ifs-posix/jobs/printf/procsub/read/redir + heredoc_huge）——需要并发管道（见下）

**验证命令**：
```bash
$env:WINUXSH_RUNNER='C:/Users/caomengxuan/repo/rubash/target/debug/rubash.exe'
& 'C:\Program Files\Git\bin\bash.exe' ../winuxsh/scripts/probe/suites/bash-tests-difftest.sh
# 或单测：读 bash-tests-diffs.txt 找具体 DIFF，用 bash -c 对照复现
```

## 三、关键根因速查（memory/compat-fixes-20260803.md 有完整版）

### 已修（本会话及之前）
- 调试钩子全套（DEBUG/RETURN trap、PS4、BASH_VERSINFO、FUNCNAME main、BASH_COMMAND）
- 参数替换栈溢出 DoS（collect_braced_parameter_name/matching_parameter_brace 的 `\` 配对）
- case/[[ ]] 模式匹配（case word 不剥引号、quoted-glob 模式匹配）
- 子 shell errexit（subshell 边界捕获 ExitCode）
- 管道特殊内建（execute_builtin_pipeline_stage + GlobalStdout thread_local 捕获）
- 进程替换嵌套命令替换（split_shell_words 的 `<(...)` 处理 + cat 特例）
- IFS 分词族（`$*` IFS 首字符、`$@` 多词、空字段）
- 数组 `+=`（arr+=str 追加 arr[0]）
- 算术错误码传播（$(( '1' )) 报错 + 跳过命令列表）
- alias 单引号/管道（expand_aliases_with_raw + execute_pipeline_command 展开）
- var-op 族（patsub `&`、嵌套 slice 顶层冒号）
- 差分 26/26（tilde 引号保护、is_assignment、BASH_COMPAT_VERSION）

### 未修（下一轮重点）
- **bash 83 的 69 DIFF**（阶段 2）——按上面分类逐项
- **挂起族**（阶段 3，P0）：
  - heredoc_huge（#26）：`yes|head|md5sum` 外部命令管道串行捕获挂起
  - coproc 挂起（#21 §2.1）：COPROC 数组存伪造 fd (0 1)，`<&"${C[0]}"` 变 `<&0` stdin 阻塞
  - **并发管道实验结论**：std::io::pipe（崩溃）、os_pipe（Windows 句柄继承挂起）、线程 io::copy（竞态不稳定）——均已回退；**需 CreateProcess 手动句柄管理**（只继承需要的句柄）或 winuxcmd 原生管道集成
- 内置族 120 项（umask 符号模式/trap ERR/kill -l/set/shopt/echo/cd/jobs）——对照 bash builtins/*.c + docs/bash-source-map.md
- 路径转换家族：run-minimal `/usr/bin/rm`、winuxcmd ls `/tmp` 映射（跨仓库）

## 四、新会话开场指导

1. 确认基线：`cargo build` + 差分 `bash tests/difftest/difftest.sh`（应 26/26）
2. 读 `docs/bash-mastery-plan.md`（阶段计划）+ 本文件 + `memory/compat-fixes-20260803.md`
3. 阶段 2：跑 winuxsh bash-tests-difftest 拿 69 DIFF 列表，按"非挂起族"逐项修（对照 bash 实际输出）
4. 每修几项跑全套回归（差分 + 上游 + Rust），避免回归
5. 完成阶段 2/3 后按计划推进阶段 4-7（内置族 → oil/mksh → busybox/ksh93 → 回归+发布）

## 五、git 状态
- 所有修复已提交到本地 master（领先 origin/master 数个 commit，未 push）
- 发布协调（阶段 7）：rubash 合并主分支 → winuxsh 用最新 rubash 打 tag
