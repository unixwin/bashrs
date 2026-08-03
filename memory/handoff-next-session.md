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
