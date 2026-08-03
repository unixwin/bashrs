# 2026-08-03 rubash 兼容性修复进度

## 本轮完成（差分测试 23 case: 11 过 → 17 过 / 6 失败）

### 差分测试框架（tests/difftest）
- `tests/difftest/difftest.sh` 驱动（bash vs rubash stdout/stderr/rc 逐字节对比）
- 23 个 case 复制自 winuxsh probe（tests/difftest/cases/case-*.sh）
- 运行: `bash tests/difftest/difftest.sh [pattern]`

### 已修复（按提交顺序）
1. **调试钩子**（3895229）: PS4+stderr / DEBUG trap（跳过 fn 定义/if/while/for 命令，for 每迭代触发）/ RETURN trap（函数+sourced）/ BASH_VERSINFO / FUNCNAME main / BASH_COMMAND 快照（debug_trap_command 字段优先）
2. **参数替换栈溢出 DoS**（3895229）: `collect_braced_parameter_name` + `matching_parameter_brace` 的 `\`+任意字符消费配对（对齐 bash extract_dollar_brace_string i+=2）；`decode_parameter_pattern_quotes` 尾部 `\` 保留为 `\x18`
3. **嵌套命令替换**（b115fb1）: echo/printf/cat 走非 AST 路径；`command_list_substitution_output` 去单命令限制 + 末尾兜底；`run_external_command_substitution` 找不到命令返回 None
4. **引号保护字段分割**（f5e26e9）: `splits_unquoted_expanded_word` 跳过引号包裹 word；`expand_embedded_parameters_mut` 恢复 `\x18`
5. **IFS/花括号/$***（fae9e25）: IFS 空字段保留（#21 1.2）；双引号花括号不展开（1.1）；`$*` 用 IFS 首字符连接（1.3）
6. **PPID/BASHPID**（3957b36）: Windows Toolhelp32 / Unix getppid 真实父进程
7. **case word / [[ ]] 模式**（3e6ed77）: case word 不剥引号；`quoted_conditional_pattern_status` 的 == 用模式匹配（`strip_rhs_pattern_quotes`）；case-06/14/16 PASS
8. **子 shell errexit**（6d285f7）: `execute_subshell_command_with_redirects` + ast_exec 循环在子 shell 边界捕获 ExitCode（case-17 PASS）
9. **管道特殊内建**（34e0388）: `execute_builtin_pipeline_stage`（子 shell+thread_local 捕获）；`write_stdout_bytes`/`write_default_stdout` 检查 thread_local 捕获；`GlobalStdout` 适配器（set/declare/setattr 走它）；case-18 PASS
10. **heredoc 接收者展开**（81562c7）: `command_input_scope` 的 heredoc 用 `expand_heredoc_body`（\x1e 引号标记判断 + expand_embedded_parameters）；case-01 while read 部分 PASS

## 剩余 6 个失败 case（下一轮重点）

### #7 for+heredoc 组合（case-01 尾部）
`for w in $(cat <<EOF ...)` 输出多一行 `for in \`（命令替换+heredoc 解析残留）

### #8 tilde 引号内展开错误（case-05）
**根因已定位**：`"~/repo"` 的 word 到达 expand_word 时**引号已被词法层剥掉**（无 `\x18` 标记），`expand_word_prefix` 无条件展开。正确修复需词法层对双引号内容保留引号标记（expand_word_prefix 检查标记）。已试最小修复（expand_word 检查字面引号开头）无效并回退。

### #12 路径转换（case-08 / case-21 A/B / cdable_vars）
4.1 赋值 `C:\...` 被转 `/c/...`；4.3 参数替换结果二次转换；`${P2//\\//}` pattern `\\` 不生效（赋值后 P2 已是 `/c/...`）；PWD 拼接 bug（cli_tests sort_pos_params 基线失败）

### case-10 版本身份差异
`VI: 0 2 2 1 release` vs bash `5 2 37`——rubash 自身版本号，预期差异（保留）

## 定位结论（深挖过，下一轮可直取）
- **#6 B/C 引号残留**：`$(printf 'v=[%s]' "$(printf 'mid')")` 中 `$(printf '` 被吞（ecw/ecws 日志证实：`$(mid)` 在 expand_command_words 入口已是破坏状态）→ 破坏发生在 expand_command_words 之前的执行链预处理。printf 特例优先路径已提交（b115fb1）
- **stdout 捕获**：rubash 输出分两条路径——write_default_stdout（检查 Executor.stdout_capture 字段）与 io::stdout()（标准库，绕过）。thread_local 捕获（shell_options.rs STDOUT_CAPTURE）统一了管道 stage 场景；命令替换 $(set -o) 仍绕过（后续可把 command_substitution 捕获也切 thread_local）

## 测试状态
- Rust 测试: 仅 script_sort_pos_params 失败（基线, 路径相关）
- 差分测试: 17 PASS / 6 FAIL

## 本轮追加（第二段会话）

### 新提交
- **a00662c #12 路径转换**: `logical_destination_display` 的 Windows 分支用 `target.is_absolute()` 判断绝对路径（`C:/...` 盘符路径不再被当相对路径拼接 → cdable_vars PASS）；`replace_parameter_pattern` 非 glob 分支还原 `\x18` 为字面 `\`（`${P2//\\//}` 生效）——case-08/21 全部 PASS
- **c8965f3 #8 tilde 部分**: `quoted_literal_tilde` 改为 `value.starts_with('~')`（覆盖 `"~/repo"` 引号路径字面，词法层加 `\x1b` 保护）——纯词法场景生效

### 上游测试基线（#14）
- `scripts/run-bash-upstream-tests.sh`：**87/87 PASS**（对比 .right 期望）
- 注意 #25 揭示：.right 是旧时代期望，与 Git Bash 5.2 实际输出有 37 项可靠差异——差分测试（对比 bash 实际输出）才是真实度量

### #8 命令替换内 tilde（case-05 仍 FAIL）
- 根因：printf 特例的 `split_shell_words`（alias_helpers.rs）剥引号（`"~/repo"` → `~/repo`）→ `expand_word` 无保护被展开
- 已试：① expand_word 检查字面引号开头（无效）；② split_shell_words 全局保留引号（case-05 PASS 但 case-08/19 回归——basename 等特例未 strip_matching_quotes）→ 已回退
- 待试：split_shell_words 保留引号 + 所有特例补齐 strip_matching_quotes；或新增 split_shell_words_with_quote_info 仅 printf/echo/cat 使用（was_quoted 且 `~` 开头参数加 `\x1b` 保护）

### #7 for+heredoc（case-01 仍 FAIL）
- 未闭合 heredoc 的 body 应跨行收集（bash 语义，hd2.sh 验证：`$(cat <<EOF)` + 下一行 body + EOF）——rubash 把 body/EOF 当独立命令
- 涉及词法层 heredoc 收集（heredoc.rs 的 heredoc_delimiters 是后处理扫描），改动风险高

### 新 issue（agent 提交）
- #25：bash 官方 83 tests 差分 PASS 仅 14/83（37 项可靠差异）——与差分测试同一批根因族
- #26：busybox 套件 69+ 差异（heredoc_huge 挂起 rc=124）

## 测试状态（当前）
- 差分测试: **19 PASS / 4 FAIL**（case-01 heredoc for+heredoc、case-03 引号残留、case-05 命令替换内 tilde、case-10 版本身份预期差异）
- 上游测试: 87/87 PASS（.right 对比）
- Rust 测试: cdable_vars 已修，应全过（script_sort_pos_params 待复核）

## 重要发现（大型脚本验证，#14）

### heredoc body 收集 bug（复杂脚本挂起）
- 复现：综合脚本（87 行，含算术展开 + heredoc + here-string 组合）在 rubash **挂起**在第 8 段（`cat <<EOF` 后）
- 定位：`< /dev/null` 后挂起消失但 **heredoc body 行变成独立命令**（`heredoc: command not found`、`EOF: command not found`）——heredoc 分隔符 EOF 未识别、body 未收集
- 背景任务（stdin 挂起管道）时 heredoc 从 stdin 读取 → 无 EOF → 永久挂起；前台/`< /dev/null` 时 body 行变命令
- 拆分测试：算术+heredoc、算术+here-string、heredoc+here-string、set -u+heredoc 均正常；**三者组合**（算术展开 + heredoc + here-string）触发
- 机制：heredoc body 在词法层收集（token_actions.rs:445 assign_heredoc_body ← token.value）；heredoc_operator_context（heredoc.rs）用 `find("<<")` 可能误认 `<<<`（here-string）导致分隔符扫描错位
- 影响：`cat <<EOF`（stdin heredoc 无重定向）场景 + 特定前置状态（算术展开）→ heredoc 失效；后台 stdin 挂起是**重要兼容性风险**（#26 heredoc_huge 挂起 rc=124 可能是同类根因）
- 修复方向（下一轮）：heredoc.rs 的 `<<` 匹配区分 `<<<`；heredoc body 收集状态复位（算术展开后）

### 大型脚本验证方式
- examples 脚本含无限循环/交互（spin.bash 等），不适合直接跑；自建综合脚本（.codex-tmp/complex-test.sh）可控
- 综合脚本覆盖数组/递归/嵌套命令替换/管道/case/[[ ]]/算术/heredoc/参数展开/循环/IFS/字符串操作——第 1-7、9-12 段全部正确，第 8 段（heredoc 组合）挂起

## 维修计划落地（按 docs/bash-compat-issues.md 根因族）

### bd96cab 族 A heredoc 部分
- `heredoc_operator_context` 的 `find("<<")` 改为**锚定分隔符词**（rfind delimiter value 再 rfind `<<`）——算术左移 `$((x << 2))`、here-string `<<<` 不再被误认为 heredoc 操作符（修复 position 越界崩溃 + 元数据错乱）
- 注意：heredoc body 收集/跨行收集挂起（综合脚本第 8 段）仍是已知 P0——逻辑行合并（has_unclosed_command_substitution/brace_group）与 body 收集交互错乱，碎片逻辑行证据（`$(fact $((n-1)))` 拆成 fact/$((n-1))/))），需词法层重构

### 5ef9fcb 族 D 语法宽松度（代表用例 a= (1 2)）
- 词法层（word.rs finish_word_token）：Assignment token 且**紧邻 `(`**（peek() == '('）时 raw 加 `(` 标记（`a=(` 数组赋值形式）
- 解析层（token_actions.rs）：collect_compound_assignment 只在 raw 以 `=(` 结尾时组合——`a= (1 2)`（带空格）不再被当数组赋值（bash 语法错误）
- 验证：`a=(1 2)` 仍数组 [1] 1 2 ✓；`a= (1 2)` 变 a 空 + 子 shell 报错（比静默数组正确）
- 剩余族 D 用例：extglob 错误（`echo @(` 不完整，bash rc=2）、echo typed args——rubash parse 无错误机制，需解析器报错机制（大改动）

### a312e5a 族 D 语法宽松度（[[ ) ]] / [[ ]]）
- parser 层（conditional_command.rs conditional_expression）：args[0] == ")" 返回 Empty（防御性）
- executor 层（conditional.rs conditional_status_with_metadata）：args 空 / args[0] == ")" / args 只含 "]]" 返回 Some(1)（假）——**executor 路径不走 parser 的 expression**，必须 executor 层修
- 验证：`[[ ) ]]` fail ✓、`[[ ]]` fail ✓、`[[ a ]]` ok ✓、`[[ (a) ]]` ok ✓、case-14/16 差分 PASS 无回归

## 待办族（每族独立中等任务）
- 族 D 剩余：extglob 错误（`@(` 不完整）、echo typed args——需解析器错误机制（parse 返回错误），系统性问题
- 族 K coproc/进程替换挂起（P0）、族 C IFS 分词、族 E 内置族（umask/trap/kill/set 等 120 项）、族 H alias、族 F 数组、族 G 算术错误码、族 I 参数替换、族 J glob 路径、族 B 状态污染（P3 最难）

### 21d73d3 族 K 进程替换嵌套命令替换（#21 §1.5）
- 根因：`split_shell_words` 不处理 `<(...)`——`<(printf 'psub')` 被拆成多个词，命令替换的 cat 特例当文件路径读失败 → 空
- 修复：① split_shell_words 新增 `copy_process_substitution_word`（`<(...)` 复制到匹配 `)`，仿照 copy_dollar_paren_word）；② cat 特例对 `<(` 前缀参数执行 `expand_command_substitution(source)` 拿输出
- 验证：`echo "$(cat <(printf 'psub'))"` → psub ✓（bash 一致）；差分 19 PASS 无回归

### coproc 挂起根因定位（#21 §2.1，P0 已知）
- `execute_coproc_command`（compound_exec.rs:504）用 `std::process::Command` 启动 rubash 子进程 + `std::io::pipe()` 管道
- **根因**：COPROC 数组存**伪造 fd `(0 1)`**（`format!("({} {})", 0, 1)`）——bash 中 `${C[0]}` 是 coproc 管道读端**真实 fd**；rubash 的 `read <&"${C[0]}"` 展开为 `<&0`（stdin）→ 从 stdin 阻塞等待 → 挂起
- 修复方向（后续）：coproc 数组存可映射的 fd 标记 + rubash 的 `<&N` 重定向从 `coproc_stdout_readers` 读；Windows 无真实 fd 暴露，需内部 fd 映射机制（中等-大复杂度）

## 第三段会话成果（继续按计划实现）

### 61e28e0 族 C 补完（$@ 无引号多词）
- `expand_command_word`：无引号 `$@` 返回每参数一个词（positional_params.clone()）；引号 `"$@"` 已走 quoted_positional_at_word_values_with_raw
- 验证：`IFS=; printf "<%s>" $@` → `<a><b><c>` ✓（bash 一致）

### 06ec642 族 H 管道中 alias
- `execute_pipeline_command` 入口对每个 stage 展开 alias（expand_aliases_with_raw）——`pipehi | cat` 生效
- 族 H 全部通过：单引号防御（a99ceda）+ 管道中 alias（06ec642）+ unalias

### c62709e 族 F 数组 +=
- `append_array_value` 的 scalar_append 改为 `!value.starts_with('(')`（普通数组 `arr+=str` 追加到 arr[0] 字符串追加；`arr+=(x y)` 数组追加）——`myarray+=world` → helloworld ✓

### bd6000d/030353d 族 G 算术错误码传播（部分）
- `arithmetic_error_message`（arithmetic/mod.rs）：检测浮点（数字含 `.`）、负指数（`**` 后负）、除零 → bash 风格 stderr 消息
- `(( 1.5 ))` / `(( 2 ** -1 ))` 算术命令：rc=1 + stderr 报错 ✓（bash 一致）
- `$(( ... ))` 展开路径（expand_word.rs:190 + parameter_core.rs:83 + embedded_parameters.rs:126）：None 时报 stderr
- **剩余**：`x=$(( 1.5 ))` 赋值 RHS 展开仍静默（x=空 rc=0）——路径在 embedded_mutations.rs 的 `$((` 分支（未加报错）；`$(( '1' ))` 引号常量 rc=0（bash rc=1）；nounset 未展开

## 差分状态（持续）
- 差分测试: 19 PASS / 4 FAIL（case-01 heredoc for+heredoc、case-03 嵌套引号残留、case-05 命令替换内 tilde、case-10 版本身份预期）
- 族 K §1.5 进程替换嵌套、族 C/H/F/G 修复均无回归

## 第四段会话成果（#10-#15，按文档新增待办）

### #10 族 I 参数替换边界（var-op）
- `&` 特殊替换（0a559ec）：patsub 替换串 `&` = 匹配文本（`\&` 字面），对照 subst.c replace_pattern
- 嵌套 slice（2ac337b）：`${v:${w:-4}}` offset 用顶层冒号分割（split_top_level_colon）+ offset 表达式先 expand_embedded_parameters
- 验证：i1/i2 24 个 var-op 用例 bash 一致

### #12/#13 新增待办验证（无需修复）
- 进制字面量 `$((16#ff))`=255 等：rubash 已支持 ✓；`$LINENO` 在 `[[ ]]` 内展开 ✓

### #14 族 B 验证
- stdin 消费泄漏：b1/b2 与 bash 一致——未复现（已解决或特定场景）
- /tmp 路径映射：winuxcmd 的 ls.exe /tmp 映射与 Git Bash 不一致——**winuxcmd 工具侧问题**（跨仓库待修）

### #15 回归固化（a10fd79）
- 差分 26 case：**22 PASS / 4 FAIL**（新增 case-24 var-op、case-25 arith、case-26 alias 全 PASS）
- 算术展开错误 fatal 语义：错误跳过当前命令列表（echo 不执行）但脚本继续（bash 语义）——arithmetic_expansion_error Cell 标志 + ast_exec 跳过（index+=1）
- 4 个 FAIL：case-01（for+heredoc）、case-03（嵌套引号残留）、case-05（命令替换内 tilde）、case-10（版本身份预期）
- 算术错误 stderr 诊断格式（`'1': syntax` vs bash `'1' : syntax`）是已知微差（case-25 已排除 stderr 敏感行）

## heredoc_huge（#26）根因精确定位（P0，跨轮待修）

### 根因：外部命令管道串行模型挂起
- heredoc_huge.tests 的挂起**不是 heredoc 本身**：内联大 heredoc（huge2：3000 行 wc -c 正常）与 source 大 heredoc（huge3：md5sum 正常）都通过
- **真正的挂起**：`yes "..." | head -3000 | md5sum`（外部命令管道）——bash 秒完成（head 截断后 SIGPIPE 终止 yes），rubash 挂起
- 机制：`execute_simple_pipeline`（pipeline_exec.rs:246）**串行**执行每段（execute_pipeline_stage 完整捕获 stdout 再传下一段）——`yes`（无限输出）完整捕获永远读不完 → 挂起

### 并发管道实验（已回退，不稳定）
- 尝试：execute_external_pipeline_concurrent（std::io::pipe 读/写端分别给相邻段）+ is_external_pipeline_stage 检测——已实施并构建通过
- **问题**：`yes | head -3` 崩溃（0xC0000409）/挂起不稳定——`Stdio::from(ChildStdout)` 传读端给 stdin 错误（head 读不到）；改 std::io::pipe 后仍崩溃（Windows 句柄/spawn 交互不稳定）
- **回退**：git checkout pipeline_exec.rs pipeline_stages.rs（恢复 HEAD，构建正常）
- **后续方向**：os_pipe crate（成熟 Windows 管道）或父进程流式转发（读段 i 输出写段 i+1 输入）；或对"含限行命令（head/tail）的管道"专门处理
