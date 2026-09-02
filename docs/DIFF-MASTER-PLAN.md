# Rubash 83 测试 DIFF 主计划（全部根因清单）

> 生成：2026-08-29。基线：worktree `agent/ifs-read-splitting` 分支
> check 模式（PASS 11 / DIFF 63 / TIMEOUT 7 / SKIP 2）。
> diff 原件：`target/issue-suites/results/check/<name>.diff`。
> 每项修复必须按 AGENTS.md 流程：最小 GNU/Rubash 探针 → 根因修复 →
> focused test → 更新本表状态。

## 修复策略总纲

DIFF 按根因族分组。同族共用一个修复，修一个族翻转一批测试。
优先级排序原则：族影响面 × 复杂度。

## 族 A：Windows 错误信息 GBK 编码泄漏（✅ 已修复，2026-08-29）

rubash 把 Windows API 的本地化错误文本（GBK 字节）直接写进输出，
GNU 对应场景是 ASCII 英文。表现：`文件存在。`、`系统找不到指定的文件`、
`系统找不到指定的路径` 乱码或中文行。

| 测试 | diff 行数 | 证据 |
|---|---|---|
| redir | 3010 | `文件存在。 (os error 80)`，且循环内反复触发导致 diff 巨大 |
| rsh | 30 | `cp: 系统找不到指定的路径。` |
| read（TIMEOUT 成员） | 尾部 | `cd: ...read9-test-38948: 系统找不到指定的文件` |

根因：错误来源是 `std::io::Error` 的 Display（Windows 上为本地化
FormatMessage 文本，GBK 编码字节被当 UTF-8 透传）。
**已修复**（commit 3e05b6e1）：`src/posix_errors.rs` 中心映射 +
全部泄漏点接线 + noclobber GNU 措辞。注：redir 的 3010 行大头实际是
`write error: Bad file descriptor` 循环（fd 生命周期语义，族 K/J 面），
GBK 仅 5 行；rsh 剩余 diff 为 restricted-shell 语义（族 F）。

## 族 B：进程退出挂起 → 重新定性（2026-08-29，详见 HANDOFF §3）

**原"输出完整但不退出"理论被实测推翻**。60s 预算实测：
builtins 已自愈（4.3s）；read 16.2s、nquote 17.6s、arith 33.5s
均正常完成——只是超过 15s harness 预算；getopts 是 getopts8.sub
无限循环（`local OPTIND` 帧状态，✅ 已修 746dd1ed，翻转为 DIFF 45/54）；
printf 是 60s+ 卡死 + GNU 基线本身 2GB 病态。

真根因（慢循环）：**每个后台任务 spawn 真实 rubash.exe 进程**
（`compound_exec.rs:56 execute_background_ast_command`，
CreateProcess ~10ms + shell 初始化 + 函数序列化），微基准：
裸循环 2000 次 1.1s；每迭代 `echo & wait` 19.5s；每迭代
`(eval … & )` 11.9s；每迭代 eval+echo 23.2s。
修复方向：后台任务进程内化（线程 + 状态隔离）或 spawn 减负；
大型专项。printf 需决策：对齐 GNU 意味着 rubash 也要写 2GB
（GNU 内联 `%.<digits>s` 无溢出校验，libc 饱和到 INT_MAX；
`*` 形式有校验，printf.def getint）。

## 族 C：IFS / 字段拆分语义

| 测试 | 状态 |
|---|---|
| ifs-posix | read 末变量算法已修（分支 agent/ifs-read-splitting）；剩余 ~3006 失败只在全量运行出现，孤立 8 种构造全部与 GNU 一致。插桩证明失败点子 Shell 内 `IFS=$ifs` 未生效（IFS 停留 split() 的 `' '`）。下一步：对 `for str` 循环渐进二分找最小触发前缀；怀疑 `set x`/`shift` 位置参数或长循环赋值隔离 |
| more-exp | 部分：`${#:}`/`${#/}` 等 bad substitution 组合 |
| posix2 | POSIX 模式 5/27 子测试（running $@、-x 负测试、变量引用、case esac） |

## 族 D：字节保真 / NUL / C0 控制字符（typed-carrier 迁移面）

> 2026-08-29 实测（family-d，attempt 2）：byte-exact 基线比对（WSL GNU Bash 5.2.21 + xxd）。
> 根因修复已落地：`src/lexer/ansi.rs` 的 `decode_ansi_c_quoted` 在 ANSI-C 字符串
> 解码后按首个 NUL 截断（GNU 词为 NUL 终止的 C 串，NUL 之后整段丢弃，如
> `$'ab\x{}cd'` -> `ab`）。直接比对 `third_party/bash/tests/*.tests`（recho 在 PATH）：
> nquote1/4/5 = 0 行差（DIFF→PASS），iquote/rhs-exp = 0 行差（DIFF→PASS）。
> quote 184→1 行差（大幅改善，余 1 行为 0x80-0xFF 可视化）。quotearray/mapfile 未翻。

| 测试 | 证据 | 根因 / 定性 | 状态 |
|---|---|---|---|
| nquote1 | v^A^A 位置漂移 | NUL 截断修复后整体对齐；C0 路径本就正确 | **PASS** |
| nquote4 | ab^@cd（NUL 处理） | `$'ab\x{}cd'` 解码含 NUL，截断为 `ab` | **PASS** |
| nquote5 | ab cd ef 引号还原 | `$a`(IFS=0x01) 截断无关；0x01 保留，整体对齐 | **PASS** |
| iquote | 控制字符可视化 | `$'...'` 含 0x7f 等，截断无影响 | **PASS** |
| rhs-exp | '\' 转义引号保留 | 与 NUL 无关；整体对齐（注：:23 `${...}` 吞文件为族 E 解析器，本环境未触发） | **PASS** |
| quote | 控制字符/引号顺序 | 184→1 行差；余 1 行为 0x80-0xFF 字节（UTF-8 管道限制，非单轮可修） | 近乎 PASS |
| quotearray | 控制字符/引号顺序 | 148 行差（结构层，0x80-0xFF + 解析器引号平衡，族 E 面） | DIFF |
| mapfile | 1 行二进制 diff | **纯 CRLF harness 伪像**（非 bug）：GNU=90x0d0a，rubash=37x0d0a+133x0a；CRLF→LF 归一化后仅行尾差异 | 跳过 |

> NUL 截断修复范围：仅 `$'...'` ANSI-C 解码（命令替换的 NUL 由
> `substitution_metadata::readback` 按设计 retain(!=0) 处理，不变）。0x80-0xFF 不可在
> Rust String 单字节承载（编码为多字节 UTF-8），属 typed-expansion-migration 有意遗留边界，
> 需重构词/替换载体为 Vec<u8> 方可彻底修复 quote/quotearray 的剩余行。

## 族 E：解析器缺口（P0——一个解析错误截断整个测试文件）

| 测试 | 现象 |
|---|---|
| posixexp2 | line 16/19 语法错误截断整文件（44 行 diff vs 40 行 GNU 全输出） |
| cond | `cond-regexp2.sub` 条件表达式构造 + invalid regexp 错误未发出 |
| braces | 命令替换内引号 EOF 错误（`command substitution: line 57/58`） |
| comsub-eof | `)` 未闭合错误措辞 + here-doc EOF 警告措辞 |
| heredoc | 192 行 diff，here-doc 边界语义 |

## 族 F：内建命令选项/校验缺失

| 测试 | 缺口 | 状态（2026-08-29 实测，WSL GNU 5.2.21 基线） |
|---|---|---|
| complete | `complete -V`/`compgen -V` 选项未实现 | 基线已对齐：GNU 5.2.21 同样拒绝 -V（invalid option），.right 即此行为；rubash 已匹配。任务原假设（GNU 接受 -V）过期，无需改。 |
| test | `test: 4+3: integer expression expected` 校验顺序 | 部分修复：错误前缀 rubash: test: 改为 ./<script>: line N: test:（test.rs），消息字面值已与 GNU 一致；剩余为更深 eval 消息（too many arguments / (: unary operator expected / ) expected, found ]）及 Windows chgrp 平台噪音。 |
| type | `type: usage` 触发条件 + not-found 措辞顺序 | 消息已匹配；剩余为 stdout/stderr 交错噪音，非真缺口。 |
| shopt | usage 触发条件 + 选项枚举顺序 | 用法/枚举消息已匹配；剩余为交错噪音 + 平台选项集差异（族 J）。 |
| appendop | readonly 变量 += 不报错；declare -A/-ai 输出 | x: readonly variable 已报；declare -A/-ai 输出格式已匹配。剩余为交错噪音。 |
| attr | readonly 导出属性输出格式 `declare -ar a=([0]="1")` | 已修复（根因）：readonly 数组重赋值错误曾泄漏函数名（f2: a: readonly variable），根因在 readonly_error_subject 返回 __RUBASH_CURRENT_FUNCTION；改为 None 后输出 a: readonly variable 与 GNU 一致。剩余为 echo -n stdout/stderr 交错。 |
| dstack | pushd/popd 错误措辞 | 三条错误措辞已匹配；剩余为 Windows 路径（C:/... vs /usr）平台差异（族 J）。 |
| func | 函数嵌套上限（GNU 100/20 分级 + 措辞）、sys$read not a valid identifier | 嵌套上限已修复（根因，曾栈溢出崩溃）：function_calls::execute_function 现按 $FUNCNEST（0=无限）/默认 100/POSIX 20 报错 <script>: line N: <fn>: maximum function nesting level exceeded (N)。func5.sub 函数名校验（sys$read / <(:) / break / !! / a b c）属解析器层函数名校验缺口，待族 E/解析器专项。 |
| exportfunc | 导出函数状态传递 | 消息已匹配；剩余为 here-doc 计数/导出函数传递的解析器层缺口（大子系统，非本族范围）。 |

## 族 G：declare/数组输出格式

| 测试 | 证据 |
|---|---|
| assoc | `declare -A`（无参）把 BASH_ALIASES/BASH_CMDS 也列出（GNU 不列）；缺 `must use subscript when assigning associative array` 校验；键序 |
| array | `unset: c[2]: not a valid identifier`（unset 数组下标校验） |
| nameref | `declare -n fee="flip"` 输出 + 多余 `two/three` 行 |
| varenv | `c=7: command not found`（赋值词被当命令） |

## 族 H：深层展开语义

| 测试 | 证据 |
|---|---|
| exp | `${_+}` bad substitution 措辞、`${xyz: ...}` 算术错误措辞 |
| new-exp | `HOME: }` 算术操作数错误 + argv 顺序 |
| comsub / comsub2 / comsub-posix | argv 顺序、POSIX 形态、多余输出 |
| extglob | extglob 状态枚举 + 模式 |
| glob-bracket | 仅 5 行：缺 `examples/loadables` 目录（harness 需复制子目录）+ shared objects 平台限制 |
| arith-for | 除零/非变量赋值错误措辞 |

## 族 I：历史/交互（平台集成，不修）

| 测试 | 证据 |
|---|---|
| history | 历史列表编号/格式 | **已跳过**：history 由 niubash（原 winuxsh）底层提供，rubash 不独立实现 |
| histexp | ! 历史展开 | **已跳过**：同上，由 niubash 底层提供 |

## 族 J：平台差异（可接受，不修）

| 测试 | 说明 |
|---|---|
| glob / globstar | 部分行依赖 bash 源码构建树（`lib/glob/smatch.o` 等）不存在于本环境；glob2.sub 的 zh_TW locale 警告属预期 |
| glob-bracket 部分 | shared objects 不支持（Windows 无动态加载） |
| intl | locale 数据本身缺失， 部分 UTF-8 双重编码疑似真 bug 待查 |
| procsub | 进程替换临时文件路径 `C:UsersADMINI~1...` 反斜杠丢失——**这个是真 bug**（路径分隔符被吃），不是平台差异 |
| invocation | `./x23: cannot execute: required file not found`（shebang/EXEC 平台面）+ SHELLOPTS readonly |

## 族 K：其他单点（需逐一最小探针定位）

| 测试 | 初步观察 |
|---|---|
| alias | `alias: 0` 多余输出、`oo: command not found` 缺失 |
| case | 多余 `fallthrough/to here/and here` 输出 |
| casemod | `${var^}`/`,}` 大小写变换在特定词上的结果 |
| cprint | 函数内 `$0` 展开差异 |
| comsub-eof | 见族 E |
| dbg-support | `caller` 行号/调试陷阱输出（830 行 diff） |
| dynvar | BASHPID/BASH_ARGV0 动态参数在特定位置展开 |
| lastpipe | `lastpipe1.sub returns 14`（lastpipe + 子 Shell 退出码） |
| posixpipe | `real 0.00`（time 内建输出混入） |
| precedence | `True/False` 函数重定义下的 `&&`/`\|\|` 输出顺序 |
| procsub | 临时文件路径分隔符丢失（真 bug，见族 J 标注） |
| redir | 见族 A（GBK）+ 重定向语义 |
| set-e / set-x | -e 传播边界 / xtrace 输出格式 |
| test | 见族 F |
| tilde / tilde2 | tilde2 已 PASS；tilde 剩余为另一 agent 管辖 |
| mapfile | 见族 D（CR 字节） |

## 已完成（避免重复劳动）

- quoted-tilde ESC 哨兵泄漏（dstack2 PASS）
- `${var:-word}` 未加引号默认词 tilde 展开（tilde2 PASS）
- read 末变量拆分 read.def 算法（分支 agent/ifs-read-splitting）
- harness stdin `</dev/null`（TIMEOUT 6/7 消除）
- `.gitattributes` 固定 `.right` 为 EOL 中性（worktree 假 DIFF 根因）
- 2026-08-29 会话（详见 `docs/HANDOFF-20260829.md`）：
  - **族 A 完成**：`src/posix_errors.rs` Win32/errno→GNU strerror 映射，全部
    Display 泄漏点接线（ExecuteError/cd/cp/spawn/脚本打开/重定向失败），
    noclobber 采用 GNU `<target>: cannot overwrite existing file` 措辞。
    redir/rsh/errors 三测试 GBK 行清零（commit 3e05b6e1）。
  - **族 B 部分并修正定性**：getopts8 无限循环根因是
    `__RUBASH_GETOPTS_OFFSET` 未随 `local OPTIND` 帧化（commit 746dd1ed），
    getopts 已 TIMEOUT→DIFF(45/54)。**"退出挂起"理论被推翻**：read 16.2s/
    nquote 17.6s/arith 33.5s 均能完成，真根因是每个后台任务 spawn 真实
    rubash.exe（~10ms/次）；printf 的 GNU 基线本身 2GB（病态，待决策）。
  - **族 C 探针就绪**：posixexp2 最小复现与 lexer 分析完成，见交接文档§4。

## 建议修复顺序

1. 族 A（GBK 泄漏）——最小修复、最大 diff 收益（redir 一项 3010 行）
2. 族 B（退出挂起）——翻转 6 个 TIMEOUT 为可测
3. 族 E（解析器 P0：posixexp2、cond）——解锁整文件
4. 族 F+G（内建校验与 declare 输出）——一批小修复
5. 族 D（字节保真，typed-carrier 面）
6. 族 C（ifs-posix 二分）
7. 族 H/K（深层展开、单点）；族 I 已跳过（niubash 平台集成）
- 2026-08-29 后续会话（AgentTeams 并行修复 + 基础设施）：
  - **基础设施工**：`.gitattributes` 追加 `*.sh eol=lf` + `*.rs eol=lf` 根治 CRLF 污染
    （`core.autocrlf=true` 会导致 WSL 测试全部报 `$'\r': command not found`）。
    `AGENTS.md` 新增 Multi-Agent / Handoff Discipline 段（CRLF 纪律、WSL 唯一基线、
    continuation.rs 独占、共享树卫生、诚实交付）。见 commit b9f129d3。
  - **族 E 部分（解析器 quote-leak）**：`src/lexer/continuation.rs` `has_unclosed_quotes`
    现跳过 `${...}`/`$(...)`/反引号/`$'...'` 为自包含单元，参数展开默认词内引号
    不再泄漏到外层引号状态。posixexp2 从 1→13 行输出（仍 DIFF 但解锁了更多测试用例）。
    commit 3285fd09。
  - **族 F（内建选项）**：3 个根因修复——attr readonly 数组错误不泄漏函数名、
    func FUNCNEST 嵌套上限（不再栈溢出崩溃）、test 诊断前缀对齐 GNU。
    commit 3285fd09。
  - **族 E 部分（分组条件）**：`src/parser/conditional_command.rs` 允许 `[[ (-n a) ]]`
    分组条件解析。cond 输出从 12→166 行。commit 36d7c1e7。
  - **族 C 部分（IFS 字段拆分）**：`src/executor/read_split.rs` IFS 分隔符字段模型改善
    （连续分隔符→空字段、尾空丢弃、末变量取剩余原始尾）。commit 36d7c1e7。
  - **族 H（${#X} 校验）**：`src/executor/parameter_errors.rs` `${#:}/${#/}/${#1xyz}`
    正确报 bad substitution。后修复回归：`${#:x}` 等合法构造不再误拒。
    commit 36d7c1e7 + 84fac064。
  - **.right 基线全部重新生成**（WSL GNU Bash 5.2.21，commit 36d7c1e7）。
  - **族 B 策略确定**：原生 Win32 无 fork，Cygwin 模拟排除，`NtCreateProcess` 方案
    对 Rust 不适配（知乎 49034308 证实）。正路=进程内线程化+per-job 状态隔离+
    job/pid 模拟层，列为后续独立大型专项，不与当前族并行。printf 走豁免清单。
  - **当前 check 基线**：PASS=11 DIFF=65 TIMEOUT=5 SKIP=2（fresh gen 后）。
  - **.right 基线已全部重新生成**（fresh gen via WSL GNU 5.2.21）。

