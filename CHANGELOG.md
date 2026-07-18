# 更新日志

所有重要的项目更改都将记录在此文件中。

格式基于 [Keep a Changelog](https://keepachangelog.com/zh-CN/1.0.0/)。

## [Unreleased]

### 构建

- CI 在推送 `vX.Y.Z` 标准版本 tag 后，会等待测试通过并自动创建 GitHub Release，支持 `v0.2.0` 这类次版本发行。

### 修复

- alias 展开为 `if ...; then`、`while ...; do`、`for ...; do` 等复合语句前缀时，会继续拼接到匹配的 `fi`/`done` 后重解析执行，补齐更接近 Bash 的 parser-level alias 行为。
- 赋值 RHS 紧邻 `<(...)` 进程替换时会作为赋值值解析并物化为可读路径，补齐 `p=<(cmd); cat "$p"` 这类 Bash 语法框架。
- 赋值 RHS 中的 `>(...)` 输出进程替换会登记为可写路径，并在后续重定向写入该路径后执行替换命令。
- `[[ -e <(...) ]]`、`[[ -p >(...) ]]` 等条件文件测试会识别进程替换操作数的 pipe/fd 形状，补齐条件命令中的进程替换文件测试框架。
- `for`/`select` 词列表中的 `<(...)`/`>(...)` 会先物化为可读/可写路径，再作为循环变量值使用。
- alias 引入复合语句前缀时，匹配 `fi`/`done`/`esac` 会按嵌套复合语句深度处理，避免内层 `if` 或循环提前截断外层 alias 语句。
- alias 引入 `time` 前缀并计时复合命令时，同样按嵌套深度收集被计时的 `if`/循环主体，避免内层结束词提前截断 `time` 语句。
- alias 引入 `case` 时，case clause body 边界会跳过内层 `case ... esac`，避免内层 `;;`/`esac` 提前截断外层 clause。
- alias 值提供 `function name` 前缀、函数体由后续 `if` 或循环复合命令提供时，会继续收集到匹配结束词后重解析为函数定义。
- 输出方向进程替换作为普通参数传给管道外部命令阶段或函数调用时，会物化为可写路径，并在阶段/函数结束后执行替换命令。
- 管道中的复合命令和函数阶段改为子 shell 风格执行，避免 `cd`、函数定义和算术赋值等状态泄漏到外层 shell。
- `builtin` 命令统一走完整内建分发表，避免无重定向调用时漏掉 `let`、`read`、`mapfile`、job control 等已接入内建命令。
- `shopt -s lastpipe` 开启后，管道最后一段可在当前 shell 执行，使 `read` 和 `while read` 等 final stage 状态更新保留到外层。
- `coproc` 默认管道的父子端接线修正，并将 coproc 子进程纳入 `wait` 可识别的作业表，避免 brace body 在 Windows 上出现 stdio 访问错误。

### 测试

- 增加 alias 值内含复合语句控制词和分号的 `if`、`while`、`for` 回归覆盖。
- 增加赋值 RHS 中独立和嵌入式输入进程替换的回归覆盖。
- 增加赋值 RHS 中独立和嵌入式输出进程替换的回归覆盖。
- 增加 `[[ ... ]]` 条件文件测试中输入/输出进程替换操作数的回归覆盖。
- 增加 `for`/`select` 词列表中输入/输出进程替换的回归覆盖。
- 增加 alias 引入的外层 `if`/`for` 内嵌套 `if`/`while` 的结束词匹配回归覆盖。
- 增加 alias 引入 `time` 前缀后计时嵌套 `if` 和嵌套循环的回归覆盖。
- 增加 alias 引入 `case` 后 clause body 内嵌套 `case` 的回归覆盖。
- 增加 alias 引入 `function name` 前缀后接 `if` 和 `for` 函数体的回归覆盖。
- 增加 `tee >(cat ...)` 管道阶段和函数参数中的输出进程替换回归覆盖。
- 增加管道 brace group 与函数阶段的工作目录隔离覆盖，并将函数定义、算术命令 pipeline 阶段预期对齐 Bash。
- 增加 `lastpipe` final `read`、final `while read`、final 赋值和 `PIPESTATUS` 状态覆盖。
- 增加 `coproc NAME { ...; }` 默认 stdout pipe 与 `wait $NAME_PID` 的回归覆盖。

## [0.2.0] - 2026-07-17

### 新增

- 补齐复合数组赋值中的未引用命令替换拆词行为，并在拆出的字段上继续执行
  pathname expansion。
- 补齐 `arr=($var)` 这类未引用参数复合数组赋值字段的 pathname expansion。
- 复合数组赋值普通元素支持 brace expansion，并遵守 `set +B` 对 brace expansion
  的关闭状态。

### 修复

- quoted parameter expansion 后不再误触发 pathname expansion，行为更接近 Bash。

- Linux/Unix 上执行无 shebang 外部脚本时，遇到 exec format error 会通过 shell
  回退执行，行为更接近 Bash。
- Unix shell 回退不再完全依赖被脚本或测试改写过的 `PATH`，会兜底查找标准
  `/bin/sh`、`/usr/bin/sh`、`/bin/bash` 和 `/usr/bin/bash`。

### 测试

- 统一测试中临时外部命令的写入逻辑，在 Unix 上自动设置可执行权限，修复
  Linux CI 中禁用 builtin 后走外部命令相关用例返回 `126` 的问题。
- GNU Bash upstream runner 本地基线更新为 `87/87` 通过。
- 将 `run-minimal` 纳入默认 upstream runner 集合。
- 收敛 upstream bridge 的重复输出逻辑，降低后续维护成本。

### 文档

- 更新 README 中的当前实现状态、builtins 覆盖、测试规模和代码结构说明。
- 更新 README 中的功能进度，说明当前实现已超过早期骨架阶段，重点转向 Bash
  兼容细节和上游测试覆盖。
- 更新 README 和 GNU Bash upstream 测试文档中的测试进度、运行命令和更新时间。

## [0.1.1] - 2024-06-11

### 增强执行器

#### 已完成

##### 执行器
- 新增内建命令: `env`, `set`, `unset`, `test`, `[`
- 添加 `redirect_err_append` 字段支持 `2>>` 重定向
- 使用 match 语句简化代码，替代函数指针

##### 测试
- 扩展执行器测试: 4 → 18 个测试
- 新增环境变量测试
- 新增命令链接测试
- 新增内建命令测试

## [0.1.0] - 2024-06-11

### 首次发布

这是一个重要的里程碑，完成了 Shell 的核心功能。

#### 已完成

##### 词法分析器 (Lexer)
- 基础分词 (单词、符号、关键字)
- 运算符识别 (`|`, `&`, `;`, `<`, `>`)
- 引号处理 (`'`, `"`)
- 变量识别 (`$VAR`, `${VAR}`)
- 命令替换 (`` `cmd` ``, `$(cmd)`)
- 花括号展开 (`{1..5}`, `{a,b,c}`)
- 注释处理 (`#`)
- 转义字符处理 (`\`)

##### 解析器 (Parser)
- AST 生成
- 简单命令解析
- 管道解析
- 分号分隔命令
- 赋值语句解析
- 重定向解析

##### 执行器 (Executor)
- 内建命令: `exit`, `echo`, `pwd`, `cd`, `export`, `true`, `false`
- 外部命令执行
- I/O 重定向支持
- 退出码处理

#### 测试

- 词法分析器测试: 33 个
- 解析器测试: 13 个
- 执行器测试: 4 个
- 单元测试: 8 个
- **总计: 58 个测试**

#### 重构

- 使用 Rust 2021 新特性 (`matches!`, `let...else`)
- 代码优化: 减少约 48% 行数
- 添加 `#[inline]` 优化

#### 文档

- README.md
- CONTRIBUTING.md
- CODE_OF_CONDUCT.md
- LICENSE (GPL-3.0)

### 待完成

- [ ] 变量展开 (`$VAR`, `${VAR}`)
- [ ] 控制流 (`if`, `while`, `for`, `case`)
- [ ] 管道实现 (真正的进程间通信)
- [ ] 函数定义
- [ ] 作业控制
- [ ] 命令历史
- [ ] 更多内建命令 (`read`, `printf`)

### 已知问题

- 管道尚未实现真正的进程间通信
- 变量展开尚未实现
- 不支持控制流语句

---

## 版本命名规则

- 主版本: 不兼容的 API 更改
- 次版本: 向后兼容的新功能
- 修订版本: 向后兼容的 bug 修复

## 链接

- [GitHub Releases](https://github.com/unixwin/rubash/releases)
- [问题跟踪器](https://github.com/unixwin/rubash/issues)
