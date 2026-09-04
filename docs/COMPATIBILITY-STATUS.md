# Rubash ↔ GNU Bash 兼容性权威状态（单一事实来源）

> 最后核对日期：2026-08-29（fresh gen + check 后续会话）
> 核对方法：用 `./target/debug/rubash.exe` 直接跑 GNU 官方测试文件
> `third_party/bash/tests/<name>.tests`，对比 GNU bash 的真实输出。
> 基线约定（2026-08-29 起生效）：语义比对一律用 WSL GNU Bash 5.2.21
> （`wsl bash`）；`D:/Git/bin/bash.exe` 在引号/转义/花括号等区域的兼容性
> 低于 rubash，不得作为语义基准。
> 注：`scripts/run-83-tests.sh` 对比脚本本身已破损（`set -u` 下算术变量未初始化，
> 满屏 `系统找不到指定的路径`），不能用于判定，故本节全部为手动真实复现。
>
> 本文件是兼容性状态的**唯一权威来源**。其余 `docs/*.md` 中带日期的分析快照
> （如 `bash-test-update-20260829.md`、`rubash-compatibility-report.md` 曾宣称
> “92%、仅 1 个 bug”）已被真实复现证伪，相关文件已于 2026-08-29 删除，
> 不再作为判定依据。

## 一、总体结论

- 简单用例层面，数组、关联数组、算术、条件、nameref、mapfile、POSIX 命令替换
  (`$(...)`)、前缀花括号 (`foo{a,b}`)、转义逗号 (`\{a,b\}`) 等**均已可用**，
  与 bash 在隔离场景下一致。
- 但跑**完整 GNU 测试文件**时，仍有若干“早期终止/大行数缺口/解析错误”的实质缺口。
- 真实剩下的高优先级缺口（2026-08-29 后续会话后）：
  `cond`、`posixexp2`、`comsub-posix`、`mapfile`、`braces`。
- 2026-09-02 会话收官（账本 41）：globstar 75→241/587（语义重构 `106a7136`：
  空 `**` 匹配一切、`**/` 仅目录加尾斜杠、递归永不穿符号链接、去 `./` 前缀；
  配对探针五构造逐字节一致）；连带修出 mkdir flag 当路径名的预存 bug（
  `606108c1`，曾被 globstar 树暴露）与每目录 DFS 排序归一（`cb92206a`，门禁
  中性）。globstar 残余 346 行根因 = check 侧 ls 为 Windows PATH 的 MSYS ls
  而 gen 侧为 WSL GNU ls（双侧二进制不对称，recho/zecho 同类 harness 缺陷），
  下一步 = run-83.sh 双侧统一 ls helper。B 兵团 fd-model 收官 `e3a9b63e`
  （exec stdio 处理 flag 被 continue 绕过的关键 bug、歧义重定向措辞、零字段
  管道段=空命令、procsub 物化；redir 175/165、procsub 33/24；其基线陈旧
  论断经 fresh gen 复现 165/24 后否决）。B 残余：exec fd 中毒块（需外部
  stdout 回放调查）、脚本自 stdin 续跑（rubash 整体缓冲）、declare -f
  序列化丢重定向文本、let 后缀自减。

## 二、真实复现结果（按严重度）

| 测试文件 | rubash 行数 | GNU bash 行数 | 严重度 | 现象 |
|---|---|---|---|---|
| `posixexp2` | 13 | 40 | **严重** | quote-leak 修复后输出 13 行（原 1 行），仍差 27 行；深层嵌套 $(... 在 ${...} 内泄漏待修 |
| `mapfile` | 170 | 170 | **严重** | 输出行数与 GNU 一致（170=170），差异为内容而非行数 |
| `cond` | 166 | 44 | **严重** | 分组条件 [[ (-n a) ]] 解析修复后输出 166 行（原 12 行），但基线 44 行为旧版 GNU 输出 |
| `comsub-posix` | 30 | 70 | 高 | POSIX 命令替换形态展开不足 |
| `braces` | 141 | 104 | 高 | 花括号展开实现缺口（见第三节） |
| `array` | 145 | 621 | 中 | 已不终止，有内容/格式差异 |
| `arith` | TIMEOUT | TIMEOUT | 中 | 已不终止，有内容/格式差异 |
| `nameref` | 40 | 40 | 低 | 行数一致，少量内容差异 |

## 三、花括号展开（`braces`）具体缺口

rubash 输出 141 行 vs bash 104 行，差异集中在 `braces.rs` / `expand_range` /
`is_brace_expansion`：

1. **前缀形式不展开**：`is_brace_expansion()` 只认“以 `{` 开头”的词，
   导致 `foo{a,b}`、`baz{x,y}` 等被当成字面（注：此点另一 agent 已修复，
   隔离用例已 OK，但完整文件仍可能因其他差异错位）。
2. **转义逗号保字面**：`{abc\,def}` 应保留字面 `{abc,def}`，rubash 误展开为 `abc def`
   （另一 agent 已修复转义逗号）。
3. **序列**：零填充（`{01..05}` → `01 02 03 04 05`）、反向序列顺序、嵌套组
   `{{0..10},x}`、错误消息措辞、展开顺序——均有差距。
4. CRLF 脚本模式下转义花括号字面输出偶发尾部空格（既有脚本模式遗留，非本次修复引入）。

## 四、已修复（避免重复劳动）

| 缺口 | 修复方 | 状态 |
|---|---|---|
| `echo {a,b}{1,2}` → `a1 a2 b1 b2`（相邻/嵌套花括号笛卡尔积） | 本次会话 | 已修（命令替换内花括号递归展开 + 相邻组 lexer 规则） |
| `echo $(echo {a,b}{1,2})`（命令替换内花括号） | 本次会话 | 已修 |
| 转义逗号 `\{a,b\}` 保字面 | 另一 agent | 已修 |
| 前缀花括号 `foo{a,b}` / `baz{x,y}` | 另一 agent | 已修（隔离用例 OK） |
| 数组 / 关联数组 / 算术 / 条件（隔离） / nameref / mapfile（隔离） / POSIX comsub（隔离） | 其他 agent | 已修 |

## 五、非缺口（不要误报）

- **平台噪音**：10 个测试 GNU bash 返回 127（找不到 `recho`/`zecho` 辅助脚本），
  rubash 反而正确执行——这不是 rubash 的 bug。
- **超时规则不同**：6 个测试双方超时计数不同，属平台差异。
- **rubash 优于 bash**：`builtins`、`comsub2`、`histexp`、`complete -p` 计数等无需修。

## 六、建议的下一步优先级

1. **P0**：`posixexp2`（整文件解析错误）、`cond`（第 54 行条件构造）。
2. **P0**：`mapfile`（管道/多行场景）。
3. **P1**：`comsub-posix`（POSIX 命令替换完整形态）。
4. **P1**：`braces` 序列零填充 / 反向 / 嵌套组 / 错误消息（system-level 补齐）。
5. **P2**：`array` / `arith` / `nameref` 的内容与格式差异；错误措辞、xtrace、环境变量泄漏等格式项。

## 七、run-83 rights 基线（2026-08-29，run-83.sh）

测试设施：`tests/gnu-compat/run-83.sh`，三模式——

- `gen`：WSL GNU Bash 5.2.21 在干净环境下生成 `tests/gnu-compat/upstream-rights/<name>.right`
  （`third_party/bash/support/{recho,zecho,printenv}.c` 用 gcc 现编译、
  CRLF 修复副本、`THIS_SH=bash`）。GNU 自身跑不完的测试记入
  `tests/gnu-compat/GNU-TIMEOUT.txt`（当前：`jobs`、`trap`）。
- `check`：rubash 对已提交 `.right`，不依赖 WSL，日常开发/回归用。
- `live`：rubash vs WSL 实时输出，排查基线本身用。

两侧 helper 实现一致（官方 C 源编译），避免 helper 分歧噪音。产物在
`target/issue-suites/results/<mode>/`。

首次 check 基线（2026-08-29，含 quoted-tilde 修复后）：**PASS 11 / DIFF 63 /
TIMEOUT 2 / SKIP 2**（83）。

### TIMEOUT 族（已解决大部分）

7 个 TIMEOUT 中 6 个是 harness 缺陷（stdin 永不 EOF，`arith`、`builtins`、
`getopts`、`nquote`、`printf`、`read` 在 `</dev/null` 下均正常完成）——
run-83.sh 已改为两侧 `< /dev/null`。剩余 `ifs-posix` 是性能（6856 个
管道+子 Shell 子测试需 60-120 秒，`RUN83_TIMEOUT=180` 可跑完）加真实失败。

### 已修（本轮）

- **quoted-tilde ESC 哨兵泄漏**：`echo expect '~1'`、`printf %q '~'` 输出
  `\x1b` 字节。根因：fully-single-quoted 快速路径未剥离 `\x1b` 标记。
  `dstack2` 转 PASS。
- **`${var:-word}` 未加引号默认词缺 tilde 展开**（原第 1 待修项）：三层修复
  ——quoted 处理器接收 context、embedded 有序展开器透传 context、
  command_prepare 对双引号 raw 的整词 `${...}` 补回 `\x1d` 标记。
  `tilde2` 转 PASS。
- **read 末变量拆分**（分支 `agent/ifs-read-splitting`）：按 read.def 实现
  ——先再提取一个字段，耗尽则取该字段，否则取整个余量仅去尾随 IFS 空白。
  18 个探针与 WSL GNU 全一致，ifs-posix 自报失败 3178→3006。

### 已定位、待修（按影响排序）

1. **`ifs-posix` 全量运行才出现的读拆分失败（~3006/6856）**：read 末变量算法
   已按 read.def 修复（见上），剩余失败只在大规模运行中出现——孤立复现
   （单行/命令替换/函数级/环境 IFS 污染共 8 种构造）全部与 GNU 一致。
   插桩证据：失败点 `IFS=[ ]`（split() 的环境值），说明子 Shell 内
   `IFS=$ifs` 赋值在全量场景未生效；孤立时同一构造正常。下一步：对
   `for str` 循环做渐进二分（截取前 N 个组合），找出最小触发前缀，定位
   泄漏状态的来源（疑似 `set x`/`shift` 位置参数或长循环下的赋值隔离）。
   注意：验证 IFS/空白必须用 `${#var}` 长度探针，行尾空格在输出显示中
   不可见，会造成误判（本轮曾因此误判"赋值 RHS 被字段拆分"，已证伪）。
2. **rubash PATH 查找在 `D:/` 风格混合分隔符下不稳定**：harness 已用 `/d/`
   风格绕开；产品侧待查。
3. **Windows 系统错误信息 GBK 编码泄漏**：找不到 `.sub` 文件时输出乱码。
4. `mapfile.right` 含 CR 字节，需字节级分析 rubash 是否做文本模式 CRLF 翻译。
5. `appendop`：readonly 变量 `+=` 不报错；`declare -A`/`declare -ai` 输出
   格式与 GNU 不一致。

## 八、权威基线约定

- 兼容性判定基线 = GNU bash 语义；本机比对一律用 WSL GNU Bash 5.2.21
  （`wsl bash`；含连续反斜杠或多层引号的 case 必须用脚本文件方式喂给双方，
  不能走 `wsl bash -c "$c"`，wsl.exe 命令行透传会把 `\\` 折叠成 `\`）。
  **不要**用 winuxsh shim；**不要**用 Git Bash（`D:/Git/bin/bash.exe`）作
  语义基准——它在部分区域兼容性低于 rubash，会得出错误结论。
- 任何“已修复/仍残留”状态变更，必须**真实跑对应 GNU 测试文件复现**后更新本文件，
  不得仅凭推断。

## 九、2026-09-01 多智能体复现与源码一致修复检查点

6 个根因族由只读调查子智能体复现并对照 GNU C 源码分析，报告位于
`docs/investigation/{posixexp2,heredoc,ifs-posix,procsub,declare-array,deep-expansion}-investigation.md`。
船长按 AGENTS.md 流程逐族验证后落盘的源码一致修复如下。

### 已落地并验证（WSL GNU 5.2.21 探针 + run-83 A/B 回归）

1. heredoc/命令替换括号平衡（`src/lexer/continuation.rs::skip_parenthesized_unit`）
   - GNU 依据：parse.y gather_here_documents / make_here_document 从输入流读取
     here-doc 体，heredoc 体对括号计数器不可见；慢路径已有 heredoc 跳过，快路径缺失。
   - 修复：快路径在 <<（非 <<< here-string）处用既有 skip_heredoc_in_chars 跳过
     heredoc 体，使体内 ) 不再误闭合 $(...)，与慢路径一致。
   - 验证：新增单元测试 heredoc_body_paren_does_not_close_command_substitution 通过；
     cargo test --lib 仅余预存失败（见末节）。comsub-heredoc 探针仍未通过——真根因在
     tokenize_with_heredocs 行累加吞掉 heredoc 体行（handoff 已标记的深层
     tokenizer/heredoc 状态协调），需独立专项，非本族单点可解。

2. procsub 路径分隔符（`src/executor/execution_misc.rs::shell_display_path`）
   - GNU 依据：subst.c process_substitute 经 make_dev_fd_filename 产出正斜杠
     /dev/fd/N，保证路径能安全过 eval/source 重解析。
   - 修复：Windows 显示路径在 drive 转换前把反斜杠归一化为正斜杠，满足 eval 安全契约。
   - 验证：探针 eval echo <(echo hi) 由 C:Users...（反斜杠被 eval 吃掉）变为
     /c/Users/.../...tmp（正斜杠、eval 安全）。A/B（回退该单行）确认 dstack/redir/procsub
     run-83 行数不变（55/416/386），无回归。run-83 procsub 仍 DIFF，因路径内容
     /c/Users/... vs /dev/fd/63 仍异——需独立 /dev/fd/N fd 抽象专项。

3. nameref 模式替换（`src/executor/expand_braced_replacement.rs::expand_braced_replacement_parameter`）
   - GNU 依据：subst.c parameter_brace_expand_word 经 find_variable /
     find_variable_nameref 顺 nameref 链取目标值后再做模式替换。
   - 修复：在 env_vars.get(var_name) 前用既有 resolved_variable_name 解析 nameref
     目标名（与 parameter_patterns.rs::parameter_pattern_scalar_value 同模式）。
   - 验证：探针 declare -n v=var; echo ${v//c/x} 由 var 变为 abxde，与 GNU 一致；
     ${v} 简单解引用仍 abcde。run-83 nameref 仍 DIFF 932/372（该族其余子项：assoc 键序、
     declare -Ai 算术等独立，未动）。

4. posixexp2 case 39 引号保留（`src/executor/command_prepare.rs`）
   - GNU 依据：参数展开结果是数据，不对其套 quote removal；仅原始词法 token 走引号移除。
   - 修复：移除对展开结果调用 remove_shell_quotes 的分支（原 word_contains_brace_group 门）。
   - 验证：探针 set -o posix 下 foo=x'a'y; echo ${foo%*'a'*} 由 x 变为 x'，与 GNU 一致。
     A/B（回退）确认 quote/quotearray/posixexp2/more-exp/comsub/case/new-exp/cond 行数全不变，
     无回归；cargo test --lib 312 passed 仅余预存 PATH-env 失败。posixexp2 仍 DIFF 40/40
     （余 case 8/9/11/12/28/29/37 属 RC-1/RC-2 深层，未动）。
### 调查完成但未落地（深层 / 高 blast-radius，留作专项）

- ifs-posix：子智能体原理论（execute_materialized_command 把独立赋值当临时）经直接探针
  证伪——简单 / 子shell / 管道 / 命令替换 / 双字符 IFS 的 IFS=:; read 形式全部通过（与
  GNU 一致）。真正失败需完整 77 行探针上下文（for+函数+set/shift+while/case+跨 480 次迭代
  反复改 IFS），属状态累积/交互问题（族 B/C「状态污染，最难，间歇性，需 debug 工具定位」），
  需 LLDB/eprintln 在完整探针上定位，非独立赋值持久化问题。**切勿**套用「独立赋值永久化」
  修复——基于已证伪理论，属补丁式且不修真 bug。
- posixexp2：RC-1（parameter_words.rs 替换词引号/反斜杠两阶段）、RC-2（未引号默认词
  反斜杠在分词前丢失）。RC-3 已落地（见上 4）。
- deep-expansion：arith-for 除零错误 token 已修（arithmetic/mod.rs:623 "0 "->"0"，探针验证
  token 与 GNU 一致）；尾部操作符空操作数诊断已修（2026-09-01 续）：
  arithmetic/mod.rs::trailing_operator_error 按 GNU expr.c readtok/evalerror 语义重建
  lasttp-suffix token——表达式以需右操作数的操作符结尾时报 `syntax error: operand expected`
  （token = 操作符起点到串尾的 suffix），其中赋值操作符（`=`/`op=`，白名单不含 `==`/`<=`/
  `>=`）左值为数字时按 expassign 报 `attempted assignment to non-variable`，左值为变量时
  报 operand expected；`7++`/`7--` 按readtok 拆成单 `+`/`-`。修复 `j=` 静默、`7<=`/`7&&`/
  `j+=`/`j==`/`j!=` 错误 token、`7==` 误报 attempted assignment、`7++`/`7--` token `++`/`--`
  →`+`/`-` 共 6 子项；探针 stderr 与 GNU 5.2.21 全对齐，A/B（禁用 hook）确认 cond/errors/
  more-exp/rhs-exp/precedence/varenv 计数不变，arith-family 预存 13 个 cli_tests 失败集合
  不变（nounset 退出码等属其他族）。新增 3 个 cli_tests 回归。仍残留：
  (a) arith-for 表达式显示/`7++ ` token 尾随空格——GNU make_cmd.c make_arith_for_command
  保留 `((...))` 原文（仅去每段前导空白），rubash 解析器 token 重组丢原文，需词法层
  ARITH_FOR_EXPRS 式原文捕获专项（parse.y:4896 parse_dparen）；
  (b) `for (( $(case x in x) esac);; ))` 头部 $()/case 解析（parse_matched_pair 语义）；
  (c) `$((…))` 求值失败后 rubash 不中止当前命令（GNU status 1 且命令不执行）——
  独立语义族；run-83 arith 族 rubash 侧 TIMEOUT 预存（arith.tests 内未定位悬挂）。
  new-exp HOME:} 为更深层解析器问题（族 E 面）。arith-for 家族 `type fx` 函数源码
  格式化差异属函数定义文本保真族，不计入本族。

### 十、2026-09-01 续：算术展开致命中止语义（(c) 项落地）

以 GNU expr.c/execute_cmd.c 实测矩阵（WSL 5.2.21 文件脚本 + 单层引号 -c 探针，
探针文件一律 Write 工具落盘防 winuxsh shim 污染；wsl bash -c 多层透传再次证伪不可信）
为基线，落地算术致命中止族：

1. **实际求值定致命**：fatality 分类不再用全新空环境重求值（状态依赖错误如
   `declare -i x; y=$((1 ? 20 : x+=2))` 在新环境消失导致漏判）。Executor 新增
   `arithmetic_last_error_category`，eval_arithmetic_command_value /
   eval_arithmetic_expansion_value / eval_conditional_arith_value_categorized 把真实
   环境的错误类别直通四个分类点（expand_word/parameter_core/embedded_parameters/
   command_prepare/assignment_expansion）。
2. **词上下文致命=放弃当前列表**：`echo $((1/0)); echo after` 不打 after（status 1），
   脚本继续下一行；ast_exec 主循环新增 ExpansionFailure 臂（守卫 loop_depth==0 &&
   function_depth==0 && 非子 shell && 非复合条件），同行余部按 command.line 跳过；
   循环体/函数体内的致命错误传播到 frame 边界（整个 for/while 放弃、函数剩余体放弃）。
3. **nounset → 127**：`set -u` 下 `((missing+1))` 与词形式 `$((missing+1))` 均退出
   127（-c）/退出脚本（文件模式 rc 1）；nounset 标志从 env_vars 迁移为
   Executor Cell（`&self` 词路径可置位）；assignment 词内 nounset 保持致命。
4. **readonly 冲突=求值失败**：`xx++` 只读冲突时求值返回 None（GNU ASSIGN_DISALLOWED
   longjmp 对齐）；case 模式词展开后检查致命并放弃整个 case。
5. **测试基线修正**：`arithmetic_empty_quoted_array_subscript_fails_outside_let` 原
   期望（cmd ctx status=1、词 ctx 致命）与 GNU 相反——GNU 两上下文均非致命（cmd ctx
   status=0、词 ctx 继续 rc 0），已按 GNU 修正。
   效果：cli arithmetic 过滤 13 失败→3；errors 族 run-83 183→181 行（GNU 168）。
   run-83 A/B（cond/errors/more-exp/rhs-exp/precedence/varenv/arith-for）无回归。

仍残留（均经 HEAD stash A/B 证实为预存，非本会话引入）：
- 词法器在 `$((…))`/算术上下文丢失引号拼写：`(( '1' ))` raw 归一为 `1`（GNU 保留至
  求值器报 operand expected）、`a[\" \"]` 引号被剥留反斜杠（GNU 得 `a[" "]` 下标 0）。
  3 个 cli_tests 待此项；需词法器 word 值/raw 对算术跨度保留原拼写专项。
- `a[""]` GNU 报 `` `a[]': not a valid identifier ``（非致命），rubash 静默按 0 处理，
  诊断缺失（语义 status/继续性已对齐）。
- run-83 arith 族 TIMEOUT：arith3.sub 中 5000 次数组自增循环后悬挂（孤立复现不成立，
  需全上下文 debug 专项）。
- 全量 cli_tests 44 失败中含大量 examples/scripts/引号类预存失败（HEAD A/B 抽证
  nested_parameter、compat 8 项均在 HEAD 同样失败）。

## 十一、2026-09-03 全量门禁与并行轨道

全量 check（upstream-rights 82 家族，RUN83_TIMEOUT=180）：**PASS 13 / DIFF 67 /
TIMEOUT 0 / SKIP 2**（2026-09-02 基线 PASS 7 / DIFF 69 / TIMEOUT 4 / SKIP 3；
原始产物 `target/full-gate.log`）。新 PASS 13 家族：comsub2, dbg-support2,
dstack2, dynvar, extglob2, extglob3, getopts, herestr, ifs, invert, mapfile,
nquote2, tilde。TIMEOUT 4→0（stdin `</dev/null` 修复；ifs-posix 给 180s 可完成）。

剩余最大真缺口（排除 intl/history 平台项）：ifs-posix 1503（状态污染，LLDB 专项），
globstar 512（**分类修正：非平台项**——tests 自建目录树；根因 `./` 前缀风格 /
`ln -s` 符号链接 / 递归语义），new-exp 368，procsub 366（`/dev/fd/N` 抽象），
dbg-support 323，redir 249（fd 生命周期，与 procsub 同属 fd 模型族），more-exp 197，
heredoc 135，quote/quotearray 166，posixexp 93（sed 解析簇 + UTF-8 载体）。

关键塌缩：builtins 458 行缺口 → 494/524（差 30）；exp → 差 13；posixexp2 →
40/40 行数相等；vredir → 差 5；comsub 基线纠正为 79/85（旧 98 行基线系毒化期产物；
gen 期 run-83.sh:90 强制 `THIS_SH=bash`）。

并行轨道 A–E 已分配（文件领地互不相交；明细见
`docs/83-TEST-FULL-ANALYSIS.md` 第九节）：A globstar（glob.rs）/ B fd 模型族
（execution_misc/redir）/ C 族H 深层展开（parameter_words/read_split）/
D iquote-quote lane（quotes/embedded_parameters，captain）/ E ifs-posix LLDB。
平台/环境伪影负面清单（env 形态 118v18、stdio 交错、/tmp 映射）见
`docs/bash-compat-issues.md` 第七节，勿当语义缺陷修。
- heredoc：上述 comsub-heredoc 的 tokenize_with_heredocs 行累加协调。
- procsub /dev/fd/N fd 抽象（fd/device model 首项）。

### 预存共享树危害（非本会话引入，未触碰）

- src/builtins/fc.rs 有 +232/-6 未提交改动（半成品 fc 重写：--help/-l/-r/-s 选项），
  可编译但使 9 个 builtins::fc::tests::* 失败。最近提交 bd0a48ec(2026-08-29) 之后由
  上一会话/智能体留下。本会话 tests.rs 改动触发全量测试二进制重编方使之暴露。按
  AGENTS.md 不 stash/回退他人半成品，仅记录；如需清理建议单独任务核实 fc 重写意图。
- executor::tests::unit_tests::export_assignment_arg_preserves_quoted_spaces 依赖 PATH
  环境净化，本机真实 PATH 导致失败，与本次修复无关。

## 十二、2026-09-03 已验证平台审计纠偏附录

本节只纠正本文件前文的分类/优先级快照，不删除或改写历史记录；以下结论来自本次已验证的 WSL GNU Bash 5.2.21 对照审计。

- **globstar**：大部分残余是实际的多重性语义，尤其是 `**/**` 的折叠/匹配，不应整体归为平台噪音或排序差异。当前仅约 8 行可归因于 harness 两侧 `ls` 排序不一致；后续应把多重性差异作为真实 globstar 工作项，并单独修正 harness 排序。
- **cprint**：差异是实质性的 builtin 函数体格式化问题（function-body formatting），不是函数内 `$0` 展开问题；保留为真实内建/格式化缺口。
- **invocation**：`SHELLOPTS` readonly 行为已经匹配 GNU，不再列为缺口。仍需处理的真实项是长选项、`BASH_ARGV0` 与 pretty-print。
- **mapfile**：mapfile 已通过验证；此前关于 CR 字节/CRLF 的缺口结论是陈旧 harness 伪像，应从待修与 P0 列表移除。
- **dstack**：`/` 解析到 Winuxsh home 是真实路径解析 bug，不是可接受的 Windows 路径差异；保留为产品修复项。
- **procsub**：路径分隔符丢失问题已修复。当前仍存在真实的多余 fd-counter 输出；`/dev/fd/N` 与 Windows 临时路径的剩余差异需和该输出问题分开记录。
- **平台归属**：`intl`、`history`、`histexp` 的差异均属平台/宿主所有，不应作为 Rubash 语义缺口追修。
- **printf**：GNU 对照会产生约 2 GiB 的病态基线输出；这属于 pathological baseline，不应按普通 diff/超时门禁解释。分类与处理规则见 `tests/gnu-compat/PATHOLOGICAL-BASELINE.txt`。

后续优先级应据此更新：globstar 多重性、cprint 格式化、dstack 根路径、procsub fd-counter、invocation 长选项/BASH_ARGV0/pretty-print 为真实工作项；mapfile、SHELLOPTS、intl/history/histexp 为已通过或平台归属项。

## 十三、2026-09-04 Globstar 多重性修复

- `src/executor/glob.rs` 已修复非相邻多个 `**` 的重复发射，以及相邻 `**` 折叠后的零深度目录尾斜杠。
- 聚焦 probe `**/a/**` 的输出数量从 111 收敛到 GNU 的 49；`**/**`、`**/**/a`、`a/**/**`、`**/**/**` 的数量保持分别为 30、15、15、30。
- 权威门禁：`MSYS_NO_PATHCONV=1 wsl bash tests/gnu-compat/run-83.sh check globstar`，结果 `PASS globstar`（rubash=587，right=587）。
- heredoc 仍不能据此关闭：`check heredoc` 当前为 `DIFF (rubash=166, right=31)`，同一行 command-substitution header 的 heredoc 仍待 lexer/token collection 专项修复。
