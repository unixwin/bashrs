# 会话交接（2026-08-04）→ 新会话从这里继续

> 本文件是**会话交接文档**：新会话打开后先读本文件 + `docs/bash-mastery-plan.md`（阶段计划）+ `memory/compat-fixes-20260803.md`（根因记录）+ `AGENTS.md`（铁律：GNU Bash C 源码是唯一权威，先查 `docs/bash-source-map.md` 映射再动手）。
> 目标（docs/bash-mastery-plan.md）：通过全部套件（差分 26 + bash 官方 83 + 上游 87 + oil 684 + mksh 436 + ksh93 46 + busybox 143），解决远程全部 7 个 issue（#20-#26）。

## 一、当前精确状态（2026-08-04 会话结束时）

| 套件 | 状态 | 说明 |
|---|---|---|
| 差分 26 case | **26/26 全 PASS** ✅ | 稳定 |
| 上游 87 | 待重评 | **upstream_scripts.rs hack 已临时禁用**（`try_upstream_scripts` 开头 `return false`），.right 对比会崩——那是 hack 假象，需真实实现重评 |
| bash 官方 83 | **无 hack 真实基线 14/69 → 已修 5 个测试** | 本会话修 appendop/strip/tilde/type2.sub(部分)+sed 支持；difftest 汇总待重跑确认 |
| Rust 测试 | 145 lib 全过；69/70 integration | script_sort_pos_params 基线失败（路径相关）；executor_tests.rs 重复 mod 已删（编译错误修复） |
| oil/mksh/ksh93/busybox | 未跑（阶段 5/6） | 大套件，按根因族推进 |

## 二、本会话重大发现与已修（对照 C 源码）

### 重大发现：upstream_scripts.rs 是 86 个测试文件名硬编码 hack（反模式）
- `src/executor/upstream_scripts.rs` 的 `try_upstream_scripts` 在 `ast_exec.rs:23` 执行任何命令前拦截，按 `__RUBASH_SCRIPT_NAME` 匹配测试名，直接 `print!` 硬编码 `.right` 输出（86 个 handler + 60KB）。
- **违反 AGENTS.md 铁律 5**。hack 拦截时测试根本没执行（stderr 空、readonly 消息进 stdout 都是 hack 假象）。
- 已删 appendop hack（测试真实执行后 PASS）；其余临时 `return false` 禁用（未提交）。
- **移除方向**：每修好一个测试的真实语义 → 删对应 handler + inline 常量 + 调用链。

### 已修（本会话提交，对照 C 源码）
- **临时赋值 +=**（73ae556）：`apply_external_environment` 跳过 append 赋值词（`a+=5 printenv a` 传已拼接值 145 而非裸 RHS 5；对齐 execute_cmd.c 前缀赋值先更新变量表）。
- **integer+compound 数组**（73ae556）：`typeset -i x; x=([0]=7+11)` → x[0]=18（compound 总是建数组，对齐 variables.c assign_array_var_from_string；旧分支把 compound 当标量算术返回 0）。
- **反引号多命令**（7e8523c）：`expand_command_substitution_inner` 对含 `;`/`&&`/`||` 的 source 走完整解析执行（`echo "" ; echo ""` 是两条命令；对齐 subst.c command_substitute）。
- **declare RHS 二次展开**（7e8523c）：`expand_declare_assignment_args` 只剥 quoted 标记不重复展开（`PPATH="$XPATH:~/bin"` 二次 tilde 展开 bug；对齐 builtins/declare.def）。
- **sed 字面替换 + 行删除**（177d31f）：`apply_simple_sed_line` 默认分支做字面替换（`s/a/B/`）；`apply_simple_sed_substitutions` 支持 `sed 1d`。
- **type 输出捕获**（177d31f）：`print_function_description` 走 GlobalStdout（thread_local 捕获），`$(type foo)` 不再泄漏；移除 describe_name 的 type*.sub hack 调用。
- **$(( )) 双引号剥除**（9ac696b）：`eval_arithmetic_expansion_value` 展开上下文剥双引号（对齐 subst.c expand_arith_string）；命令上下文（for (( ))）保留报错。
- **winuxsh difftest 驱动修复**（未提交，winuxsh 仓库）：THIS_SH=/usr/bin/bash + MSYS2_ENV_CONV_EXCL + recho 工具（`../winuxsh/scripts/probe/suites/tools/recho.exe`，rustc 编译）——之前 69 DIFF 里大量是驱动缺 THIS_SH 的假失败。

## 三、下一步确切第一步（阶段 2：bash 官方 83 tests，无 hack 真实执行）

1. 确认 hack 禁用状态：`src/executor/upstream_scripts.rs:26` 的 `return false`（TEMP）——**决定**：保留（修真实语义方向）或还原。
2. 跑 winuxsh difftest 拿无 hack 真实 DIFF 列表（之前 14/69，已修 5 个应 ~18/65）。
3. **按 diff 行数从小到大批量修**（先跑 `diffsize` 排序，修接近通过的）。
4. 每修好一个测试 → 删对应 upstream handler（try_upstream_scripts 链 + handlers_*.rs 函数 + inline_*.rs 常量）。
5. **已定位未修**：
   - type2.sub 的 `eval "$(type foo | sed 1d)"` 泄漏 heredoc 内容（bar/qux 多输出一次）——eval 重解析时 heredoc 处理问题（对照 parse.y/subst.c）。
   - func.tests：函数定义格式（`function f3 ()` vs `f3 ()`）、declare -f 非法函数名输出、返回值（201 vs 116 行）。
   - casemod.tests：${x^^}/${x,,} 多参数输出错位。
   - posixpat.tests：brackpat 的 dangling backslash——词法层剥掉 case 模式里转义反斜杠导致 bracket 误闭合（pattern.rs 的转义成员语义已修 dc0b9c4，但词法层剥反斜杠使修复不触发，需词法层保留）。
   - 挂起族 rc=124（6 个）：ifs-posix/jobs/printf/procsub/read/redir——bash 侧也 124（Git Bash 环境问题），需并发管道（CreateProcess 手动句柄）。

## 四、验证命令

```bash
# 差分（应 26/26）
bash tests/difftest/difftest.sh
# bash 官方 83 tests（对照 bash 实际输出；hack 禁用时是真实执行基线）
$env:WINUXSH_RUNNER='C:/Users/caomengxuan/repo/rubash/target/debug/rubash.exe'
& 'C:\Program Files\Git\bin\bash.exe' ../winuxsh/scripts/probe/suites/bash-tests-difftest.sh
# 单测（模拟 difftest 环境：THIS_SH + PATH 含 tools 与 Git Bash bin）
cd third_party/bash/tests && R="C:/.../rubash.exe"
PATH="C:/Users/caomengxuan/repo/winuxsh/scripts/probe/suites/tools:C:/Program Files/Git/bin:$PATH" THIS_SH="$R" timeout 30 "$R" <test>.tests
# Rust
cargo test --lib   # 145 全过
# 上游（hack 禁用时 .right 对比会崩，需真实实现重评）
scripts/run-bash-upstream-tests.sh
```

## 五、git 状态
- 本会话提交：9ac696b（$(( ))）、73ae556（appendop）、7e8523c（strip/tilde）、177d31f（sed/type）
- **未提交**：upstream_scripts.rs 临时 `return false`（hack 禁用，待决策）、Cargo.lock、run-bash-upstream-tests.sh（会话前已有）
- 领先 origin/master 11 commit，未 push；发布协调（阶段 7）：rubash 合并主分支 → winuxsh 用最新 rubash 打 tag

## 六、下一会话（2026-08-04 第二段）新增成果

### 本段提交（对照 C 源码）
- **c0d2f5b case 模式 raw 反斜杠**：`case_command.rs` 用 raw token 文本构建模式（bash execute_cmd.c：模式不剥引号），`\]`/`\"`/`\\` 保留；删除 `mark_case_pattern_literal_backslashes`（错误设计：`\x18` 被当字面反斜杠）；`pattern.rs` bracket 内 `\`/`\x18` 后 `]` 是字面成员不闭合（glob.c parse_bracket）；`expand_case_pattern` 保护 `\` 过 decode 并还原 → posixpat ok 21 + case-06 C-MATCH
- **c0d2f5b 同批**：`expand_case_pattern` 改用 `expand_word_mut`（&mut）→ case 模式 `$((x=1))` 赋值副作用保留（case.tests `;&` fallthrough 输出 1.0，之前 0.1）
- **e85248d comsub 栈溢出 P0**：`expand_word_mut` 把含 `$(` 的完整 `${...}`（如 `${foo:-$(echo x)}`）路由到 `expand_embedded_parameters_mut` → `${` 内嵌分支 collect 同串再 `expand_word_mut` 无限递归。修复：完整 braced word 先走 `expand_braced_parameter_word`（对齐 &self 版本）。comsub.tests rc 0/127 → 0/0
- **b478b08 comsub 多命令**：`$(echo mn; echo op)` 首词 echo 被 specialized 快捷路径吞（`;` 当参数输出 `mn; echo op`）。修复：words 含 `;`/`&&`/`||` 先走 `run_ast_command_substitution`。comsub.tests diff 40→20 行
- **c43cfae ANSI-C `\x{hex}`**：decode_ansi_c_quoted 支持 `\x{...}`（strtrans.c：消费 hex 到非 xdigit/`}`，&0xFF，空 `\x{}`→NUL）。nquote4 核心展开修复；剩余是 recho 字节/UTF-8 显示差异（winuxsh 工具侧）

### bash 官方 83 当前状态（本段快照）
- difftest 全量卡在挂起族（timeout 未生效，rubash 进程 8 分钟不结束）→ 已取消，未拿到新汇总
- 已修测试净效果：posixpat brackpat ok 21、case.tests 剩 readonly 算术错误（xx++ 报错缺失）、comsub.tests rc 0/0
- 下一批候选（rc=0/0 行数接近）：dbg-support2（lineno 全 1，BASH_LINENO 行号跟踪）；nquote/nquote1/iquote/set-e（printf %q 等）

### 未提交
- memory 本文件、Cargo.lock、run-bash-upstream-tests.sh（会话前）
- upstream_scripts.rs `return false` hack 禁用（待决策：保留修真实语义或还原）

### 本段追加提交（第三段）
- **37122bf DEBUG trap LINENO**：ast_exec 触发 DEBUG trap 前 `set_current_line(command)`；触发条件加 `!debug_trap_running`（trap action 内命令不再重入触发，避免用合成行号覆盖 LINENO）；`execute_command` 在 trap action 期间跳过 set_current_line。dbg-support2 `$1` 参数 29/30/31... 对齐 bash；剩余 `($LINENO)` 函数内 LINENO（bash=函数定义行 18，rubash=外层值）待修（函数调用 LINENO 基准）
- **37122bf CLI `-ce` 合并**：main.rs 分解 `-Xc` 短选项为 `-X` + `-c`（c 放最后消费 argv；bash getopt 语义）。set-e2.sub 的 `${THIS_SH} -ce '...'` 不再命令找不到；set-e.tests rc 0/0（剩余 errexit 语义：`(exit 17)`、函数/管道内 set -e 行为、`ok` 缺失）

### 第三段已验证基线
- 差分 26/26 ✅；lib 145；lexer 69；executor case 66
- bash 官方 83 已修测试净效果：posixpat(ok 21)、case(剩 readonly 算术)、comsub(rc 0/0，剩 4 处细节)、nquote4(核心)、set-e(剩 errexit 细节)、dbg-support2($1 对)
- 挂起族（rc=124×6）：timeout 在 Windows 不杀 rubash 进程——difftest 全量会卡，需先修管道并发（os_pipe + from_raw_handle 已验证方案）或用 taskkill 包装

### 第四段提交（errexit 系列）
- **3a8de7d 命令替换 errexit**：`run_ast_command_substitution` 用 `with_errexit_suppressed` 执行（bash：`$(false; echo ok)` 内 false 不 abort）；POSIX 模式例外（`set -o posix; z=$(false;echo posix foo)` 退出，set-e1.sub）
- **bfc89ce 子 shell + 管道 + `$-`**：
  - `execute_command` 对 subshell 命令加 errexit 检查（`(exit 17)` 在 set -e 下退出；&&/||/! 上下文已在 ast_exec 抑制）
  - 管道非最后段 `with_errexit_suppressed`（`{ false; echo foo; } | cat` 输出 foo）
  - `shell_option_flags` 按 bash flags.c shell_flags[] 顺序（`$-` 输出 ehB）
- **剩余（set-e.tests 4 行 diff）**：① `&&`/`||` 列表**最后执行命令**失败要触发 errexit（`true && true|false`、`false | echo foo | false` 的管道最后段）——当前整个 and_or 被 with_errexit_suppressed 粗粒度抑制，需按 execute_cmd.c 粒度重构；② `{ false; echo foo; } | cat` 的 foo 与 after brace pipeline 输出顺序；③ A/B/C 一组错位
- **set-x.tests 待修**：`for ((...))` 算术头 xtrace 缺失（execute_arithmetic_for_command 需加 trace_arith_expression）；`(( ))` 内 `i>0`/`i<=5` 被词法层加空格（`(( i>0 ))` → words `["((", "i > 0", "))"]`，parse_arithmetic_command 用 tokens join）——bash 保持原样，需词法层 `((` 命令按算术上下文收集；bash 5.2 实测 `((  expr  ))` 双空格 vs 5.3 .right 单空格（difftest 基准 Git Bash 5.2）

### 本会话累计（提交 c0d2f5b → bfc89ce 共 12 项）
case 模式 raw 反斜杠 / fallthrough 算术副作用 / comsub 栈溢出 P0 / comsub 多命令 / ANSI-C `\x{hex}` / DEBUG trap LINENO / CLI `-ce` / errexit 命令替换 / errexit 子 shell+管道 / `$-` 顺序 / posix 模式 errexit / 差分 26 全 PASS 稳定
