# Rubash Builtin 与 Fast-Path 命令清单（单一事实来源）

> 最后核对日期：2026-09-08
> 核对方法：静态审计 `src/executor/builtin_names.rs`（`is_shell_builtin_name`
> 白名单）、`src/executor/command_dispatch_primary.rs`、
> `src/executor/command_dispatch_late.rs`，并与 GNU Bash 5.2（61 个 builtin）
> 以及 winuxcmd（`unixwin-winuxcmd`，176 个外部命令）做重叠比对。
> 与 GNU Bash 的**行为兼容性状态**见 `docs/COMPATIBILITY-STATUS.md`；本文件
> 只回答"哪些命令是 builtin、哪些是 fast-path、为什么"。
>
> 本文件由 `src/executor/builtin_names.rs` 内的文档同步测试守护：
> builtin 白名单或 fast-path 分支变更而没有更新本文档时，`cargo test` 会失败。
> 维护规则：**改代码时同步改下面的机器可读清单块，反之亦然。**

## 一、三层分发结构

rubash 对一个命令词按优先级做三层处理：

1. **真 builtin**：在 `is_shell_builtin_name` 白名单内，由
   `command_dispatch_primary.rs` / `command_dispatch_late.rs` 的 match 分支
   直接在进程内执行，不创建子进程。
2. **Fast-path builtin（隐藏）**：**不在**白名单内，仅在参数形态简单时进程内
   快速执行；复杂参数回退到外部命令（由 winuxcmd 提供）。这是 Windows 下
   减少进程创建开销的优化。introspection（`type`、`command -v`、`enable`）
   将其报告为外部命令——与 GNU Bash 的报告结果一致（bash 中这些本来就是
   外部命令）。
3. **外部命令**：沿 PATH 查找，由 winuxcmd 提供。

## 二、真 builtin 清单（64 项）

GNU Bash 5.2 的 61 个 builtin **全部覆盖，无缺失**；下表加粗的是 rubash 的
有意扩展（`env`、`setopt`、`unsetopt`），另有 Windows 专属 `sudo` 不计入 64。

<!-- builtins-list
.
:
[
alias
bg
bind
break
builtin
caller
cd
command
compgen
complete
compopt
continue
declare
dirs
disown
echo
enable
env
eval
exec
exit
export
false
fc
fg
getopts
hash
help
history
jobs
kill
let
local
logout
mapfile
popd
printf
pushd
pwd
read
readarray
readonly
return
set
setopt
shift
shopt
source
suspend
test
times
trap
true
type
typeset
ulimit
umask
unalias
unset
unsetopt
wait
-->

| 类别 | 命令 |
|---|---|
| Bash 61 个 builtin（全部对齐） | `.` `:` `[` `alias` `bg` `bind` `break` `builtin` `caller` `cd` `command` `compgen` `complete` `compopt` `continue` `declare` `dirs` `disown` `echo` `enable` `eval` `exec` `exit` `export` `false` `fc` `fg` `getopts` `hash` `help` `history` `jobs` `kill` `let` `local` `logout` `mapfile` `popd` `printf` `pushd` `pwd` `read` `readarray` `readonly` `return` `set` `shift` `shopt` `source` `suspend` `test` `times` `trap` `true` `type` `typeset` `ulimit` `umask` `unalias` `unset` `wait` |
| **扩展：zsh 兼容** | `setopt` `unsetopt`（配套 `src/builtins/zsh_options.rs`） |
| **扩展：进程内 env** | `env`（`executor/external_inner.rs`，按 coreutils 语义实现：排序输出、`-0` 等；bash 中为外部命令） |
| Windows 专属 | `sudo`（`#[cfg(windows)]`，不计入白名单常量） |

说明：

- `declare` 与 `typeset` 同名同实现；`source` 与 `.` 同名同实现；`[` 由
  `src/builtins/test.rs` 实现并校验闭合 `]`。
- `time`、`[[`、`((` 是**保留字/语法结构**，不是 builtin（bash 同），见
  `is_shell_keyword` 与 late dispatch 对应分支。`time` 支持 `TIMEFORMAT`。

## 三、Fast-Path（隐藏 builtin）清单

以下命令在参数形态简单时进程内执行，否则回退外部 winuxcmd 命令。
**有意不进 `is_shell_builtin_name` 白名单**，以保持 introspection 与 bash 一致。

<!-- fastpath-list
sleep
dirname
basename
-->

| 命令 | fast-path 实现位置 | 回退行为 | 备注 |
|---|---|---|---|
| `sleep` | `builtins/sleep.rs`（`can_execute_fast_path`） | `execute_external` | 支持小数秒（f64） |
| `dirname` | `executor/printf_path_builtins.rs` | `execute_external`（winuxcmd `dirname`） | 非平凡路径回退 |
| `basename` | `executor/printf_path_builtins.rs` | `execute_external`（winuxcmd `basename`） | 同上 |

`env` 是唯一的混合形态：它在白名单内（属第二节真 builtin，`type env` 报告为
builtin），但实现是进程内执行 coreutils 语义、**没有外部回退路径**。它不进
本清单，因为白名单归属与 fast-path 判定互斥，文档同步测试会拒绝两边都出现
的名字。

另有两类仅用于测试兼容的复刻 builtin（bash 官方测试套件 `tests/builtins/` 自带
同名辅助 builtin，正式发行版不包含）：`recho`、`zecho`（late dispatch）。
它们不属于 fast-path，也不在白名单内。

## 四、与 winuxcmd 的重叠

winuxcmd（176 个外部命令）与 rubash 内置/保留字重叠的命令共 13 个：

`echo` `env` `kill` `printf` `pwd` `test` `[` `true` `false`（真 builtin）、
`sleep` `dirname` `basename`（fast-path）、`time`（保留字 ↔ winuxcmd `time`）。

重叠遵循 bash 语义：脚本内调用被 builtin/保留字遮蔽；`command env`、
`/usr/bin/env` 等显式形式仍走 winuxcmd 外部版本。2026-09-08 已对重叠命令的
行为（选项、转义、格式化、信号、jobspec、TIMEFORMAT）完成逐项比对，结论：
对齐。详细判定见工作区审计报告。

## 五、维护清单（改这里之前先看）

新增/修改 builtin 或 fast-path 时：

1. 改 `src/executor/builtin_names.rs`（白名单或新分支）。
2. 同步更新本文件的 `builtins-list` / `fastpath-list` 机器块。
3. 跑 `cargo test --lib builtin_names`，文档同步测试必须绿。
