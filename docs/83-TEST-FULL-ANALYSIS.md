# 83-Test 完整分析（2026-09-02）

> 基线：PASS=7 / DIFF=69 / TIMEOUT=4 / SKIP=3
> .right 从 WSL GNU Bash 5.2.21 重新生成

## 一、总览

| 分类 | 数量 | 说明 |
|---|---|---|
| PASS | 7 | 已通过 |
| TIMEOUT | 4 | 超时（arith, ifs-posix, nquote, read） |
| SKIP | 3 | 跳过（jobs, printf, trap） |
| **DIFF 总计** | **69** | 需要分析 |
| ├ 平台差异（不修） | 8 | 依赖 Linux 特有功能 |
| ├ 小差异（≤5行） | 6 | 接近通过，微调即可 |
| ├ 已知根因族 | 30 | 已有调查报告，根因明确 |
| └ 新根因 | 25 | 需要深入调查 |

## 二、平台差异（8 个，不修）

| 测试 | 差距 | 原因 |
|---|---|---|
| `cprint` | 28行 | `/dev/fd/` 引用，`cat -` 管道行为 |
| `glob-bracket` | 1行 | `shared objects not supported`（Windows 无动态加载） |
| `globstar` | 560行 | 依赖 bash 源码构建树 `lib/` 目录 |
| `histexp` | -49行 | `!` 历史展开由 niubash 底层提供，非 rubash 实现 |
| `history` | 193行 | 历史列表编号/格式由 niubash 底层提供 |
| `intl` | 1313行 | locale 数据缺失，UTF-8 双重编码 |
| `invocation` | 80行 | shebang `./x23: cannot execute`，`SHELLOPTS` readonly |
| `mapfile` | 0行 | CRLF 字节计数差异（二进制 diff，非语义） |

## 三、小差异（6 个，接近通过）

| 测试 | 差距 | 现象 | 修复方向 |
|---|---|---|---|
| `appendop` | 7行 | readonly 错误位置偏移 + 数组输出顺序 | 调整 readonly_error_subject 顺序 |
| `attr` | 15行 | 函数调用后多余 `declare` 输出 | 函数定义文本保真 |
| `case` | 1行 | `hi2`/`1.0` 输出顺序交换 + 控制字符前缀差 | 输出顺序调整 |
| `herestr` | 0行 | `double"quote` vs `doublequote`（1行内容差） | here-string 引号处理 |
| `ifs` | 0行 | `a b c d e` vs `} x`（1行内容差） | IFS 字段拆分边界 |
| `tilde` | 0行 | PATH 展开 1 行重排 | 输出顺序 |

## 四、已知根因族（30 个）

### 族 E：解析器缺口（7 个）

| 测试 | 差距 | 现象 |
|---|---|---|
| `arith-for` | 16行 | 缺少算术错误消息（除零、语法），多余函数输出 |
| `braces` | 55行 | 缺少命令替换 EOF 错误；`zecho: command not found`（测试依赖） |
| `comsub-eof` | 7行 | here-doc EOF 警告位置错误，缺少 `)` 匹配错误 |
| `cond` | -122行 | 缺少 `cond-regexp2.sub` invalid-regexp 错误；分组条件多余输出 |
| `posix2` | -15行 | `bash: command not found` + POSIX eval/case 失败 |
| `posixexp` | 219行 | 参数展开测试多余输出；`/var/tmp/sh` 未找到 |
| `parser` | 1行 | `./bash: command not found`（测试调用 `./bash` 不存在） |

### 族 H：深层展开语义（7 个）

| 测试 | 差距 | 现象 |
|---|---|---|
| `exp` | 29行 | 缺少 `${_+}` bad-substitution 错误，多余 `expect`/`argv` 输出 |
| `more-exp` | 207行 | 缺少 `${#:}` bad-substitution 错误，IFS 字段拆分 |
| `nquote1` | 1行 | NUL 截断残余：`v^A^A` → 空，`uv^A^A` → `^]([0` |
| `nquote3` | 0行 | NUL 截断：`uv^A` → `uv^A^Awx`（ANSI-C 解码包含尾部字节） |
| `nquote4` | 0行 | 字节表示：`ab�` vs `abÞ`（UTF-8 vs 原始字节可视化） |
| `nquote5` | 2行 | 字段拆分：NUL 在 IFS 中时 `ab cd ef` vs `ab`/`cd`/`ef` |
| `rhs-exp` | 3行 | 转义引号处理：`\&` vs `\\&`，缺少 `$selvecs` 展开行 |

### 族 F：内建命令选项/校验（5 个）

| 测试 | 差距 | 现象 |
|---|---|---|
| `complete` | -70行 | `compgen -V`/`-r`/`-D`/`-z` 错误消息不同 |
| `getopts` | -17行 | 错误消息不同（`illegal option` vs `option requires an argument`） |
| `shopt` | 88行 | 选项枚举顺序不同；shebang `required file not found` |
| `test` | -137行 | 错误消息前缀不同，更深的 eval 消息缺失 |
| `type` | -29行 | 多余 `-r`/`notthere` 错误行；重复输出 |

### 族 G：declare/数组输出格式（2 个）

| 测试 | 差距 | 现象 |
|---|---|---|
| `varenv` | -568行 | `declare` 输出格式，`c=7: command not found`（赋值词被当命令） |
| `vredir` | -53行 | 错误消息 `cannot assign fd tbar`（截断）vs `cannot assign fd to variable` |

### 族 D：字节保真（1 个）

| 测试 | 差距 | 现象 |
|---|---|---|
| `redir` | -254行 | GBK 已修；剩余：fd 生命周期（`Bad file descriptor` 循环），`/etc/passwd` 未找到 |

### 族 K：其他单点（2 个）

| 测试 | 差距 | 现象 |
|---|---|---|
| `dstack` | -6行 | Windows `C:/Users/...` 路径 vs `/usr`（平台差异） |
| `casemod` | 2行 | `${var^}`/`${var,}` 大小写变换输出顺序交换 |

## 五、新根因（25 个，需深入调查）

### 高优先级（影响大或有明确线索）

| 测试 | 差距 | 现象 | 初步判断 |
|---|---|---|---|
| `builtins` | 458行 | rubash 输出完整内容，GNU 抑制 | 执行流分歧，可能与 `set` 选项或 subshell 行为有关 |
| `func` | 51行 | 缺少函数名校验错误（`sys$read`、`<(:)`、`break`、`!!`） | 解析器函数名校验缺口 |
| `errors` | -15行 | 缺少 `not a valid identifier` 错误消息 | 诊断消息缺失 |
| `set-x` | 7行 | xtrace 输出格式不同（缩进、`+` 前缀顺序） | xtrace 格式化 |
| `set-e` | -6行 | 缺少 `-ce`/`-c` 命令不存在错误 | set -e 传播边界 |
| `dynvar` | -5行 | 多余 `-c: command not found` 错误 | 动态变量展开 |
| `alias` | 94行 | 多余 `alias: 0` 输出，`\.\` 路径前缀 | alias 内建输出格式 |

### 中优先级

| 测试 | 差距 | 现象 | 初步判断 |
|---|---|---|---|
| `coproc` | 1行 | coproc PID 值错误，中文 `cat：` 标点 | 进程替换 + GBK 残余 |
| `dbg-support` | 317行 | caller/LINENO/调试跟踪输出顺序差异巨大 | 调试支持系统性差异 |
| `exportfunc` | 4行 | `-c: command not found`，缺少 here-doc/export 错误消息 | 环境调用格式 |
| `lastpipe` | 5行 | 退出码不同（`returns 14` → `returns 0`），IFS 字段合并 | lastpipe + 子 Shell 退出码 |
| `posixpat` | 12行 | 缺少模式匹配测试，多余 `oops -- bad range` 输出 | POSIX 模式匹配 |
| `posixpipe` | -29行 | 多余 `1`/`a` 行，`time` 输出格式不同 | time 内建输出 |
| `precedence` | -32行 | `}cmd1: command not found`（反引号展开差异），缺少 `Say`/`Truth` 输出 | 优先级/展开顺序 |
| `strip` | 1行 | 所有输出是 `$v`（字面）vs GNU 空/空白字符串 | 单引号内参数展开 |
| `glob` | -112行 | `read_split.rs:109` 对 `α`（多字节字符边界）panic | 多字节字符处理 |

### 低优先级 / 需进一步调查

| 测试 | 差距 | 现象 | 初步判断 |
|---|---|---|---|
| `rsh` | 2行 | GBK 已修；`/bin/sh` 未找到，中文 `cat：` 标点 | 受限 Shell + 平台 |
| `herestr` | 0行 | `double"quote` vs `doublequote`（1行） | here-string 引号 |
| `case` | 1行 | 输出顺序微调 | 小调整 |
| `appendop` | 7行 | readonly 错误位置偏移 | 小调整 |
| `attr` | 15行 | 多余 `declare` 输出 | 函数定义保真 |

## 六、修复计划（更新版）

### 第一批：已知根因，可直接修复（30 个测试）

| 优先级 | 族 | 目标测试 | 预期DIFF减少 |
|---|---|---|---|
| P0 | 族 E | posixexp2, comsub-posix, heredoc, comsub-eof, cond, posixexp, parser, arith-for, braces, posix2 | ~400行 |
| P1 | 族 H | iquote, new-exp, comsub, comsub2, extglob, exp, more-exp, nquote1/3/4/5, rhs-exp | ~500行 |
| P2 | 族 F | complete, getopts, shopt, test, type | ~350行 |
| P2 | 族 G | varenv, vredir, assoc, array, nameref | ~900行 |
| P3 | 族 D | quote, quotearray, redir | ~400行 |

### 第二批：新根因，需调查后修复（25 个测试）

| 优先级 | 测试 | 预期DIFF减少 | 调查方向 |
|---|---|---|---|
| P0 | builtins | 458行 | 执行流分歧，检查 `set` 选项和 subshell |
| P0 | func | 51行 | 函数名校验（解析器层） |
| P0 | glob | 112行 | 多字节字符 panic（read_split.rs:109） |
| P1 | errors, set-x, set-e, dynvar, alias | ~130行 | 诊断消息/格式调整 |
| P1 | coproc, exportfunc, lastpipe, posixpat, posixpipe, precedence, strip | ~100行 | 各自独立调查 |
| P2 | dbg-support | 317行 | 调试支持系统（大工程） |
| SKIP | rsh | 2行 | 受限 Shell + 平台，投入产出比低 |

### 第三批：小差异，微调即可（6 个测试）

| 测试 | 预期DIFF减少 | 修复方向 |
|---|---|---|
| appendop, attr, case, herestr, ifs, tilde | ~30行 | 输出顺序/格式微调 |

### 不修（8 个平台差异）

cprint, glob-bracket, globstar, histexp, history, intl, invocation, mapfile

## 七、预期收益

| 批次 | 覆盖测试数 | 预期DIFF减少 | 预期新PASS |
|---|---|---|---|
| 第一批（已知根因） | 30 | ~2550行 | +10~15 |
| 第二批（新根因） | 25 | ~1100行 | +5~10 |
| 第三批（小差异） | 6 | ~30行 | +3~6 |
| **总计** | **61** | **~3680行** | **+18~31** |

修完后预期：**PASS=25~38 / DIFF=38~51 / TIMEOUT=4 / SKIP=11**

## 八、注意事项

1. **族 E 修一个翻一批**：posixexp2 的解析器修复可能同时解锁 cond、comsub-eof 等
2. **glob panic 是阻塞项**：多字节字符导致 crash，影响 glob 测试
3. **builtins 是最大的单个差异**（458行），需要优先调查
4. **dbg-support 317行差异是系统性问题**，可能需要大重构
5. **平台差异的 8 个测试不要浪费时间修**

## 九、2026-09-03 全量门禁复测与根因再分配

全量 check(upstream-rights 82 个有基线家族,RUN83_TIMEOUT=180):
**PASS 13 / DIFF 67 / TIMEOUT 0 / SKIP 2**(对照本文档基线 PASS 7 / DIFF 69 /
TIMEOUT 4 / SKIP 3)。原始产物 `target/full-gate.log`。

新增 PASS(13): comsub2, dbg-support2, dstack2, dynvar, extglob2, extglob3,
getopts, herestr, ifs, invert, mapfile, nquote2, tilde。

### 关键缺口塌缩(文档值 → 2026-09-03 现值)

- builtins 458 行缺口 → **494/524(差 30)**——文档最大单点已被后续会话吃掉大半
- exp 29 行 → 520/533(差 13);vredir → 118/123(差 5);varenv → 381/402(差 21)
- posixexp2 13/40 → **40/40 行数相等**(剩内容差)
- comsub 基线纠正:79/85(旧 98 行基线系毒化期产物;gen 期 run-83.sh:90
  强制 `THIS_SH=bash`,GNU 侧子脚本不受 Windows 环境污染)
- func 51 行 → 230/193(THIS_SH 毒化修复后 func5 完整跑通,残差 5 类见账本)
- glob panic(read_split.rs:109 多字节)已修,P0 阻塞解除

### 分类修正:globstar 不是平台差异

本文档第二节将 globstar(560 行)归为平台差异("依赖 bash 源码构建树 lib/ 目录")
系误判:globstar.tests **自建 GDIR 目录树**(mkdir lib builtins 等,第 26-39 行),
不依赖任何外部构建树。真根因三点:(a) rubash `**` 递归展开产生 `./` 前缀风格
(GNU 输出相对路径);(b) `ln -s a c` 符号链接目录的展开;(c) 递归深度/去重语义。
可修,预期收割 ~512 行。

### 新增归档的平台/环境伪影(勿当语义缺陷修)

- **env 形态**:rubash 导出 118 变量 vs WSL GNU 18(wsl.exe 只把 WSLENV 指定
  变量送入 Linux 侧;interop 启动 Windows 二进制携带宿主完整环境,`env -i`
  无效)。影响任何使 declare -p 退化为无参的用例(nameref ~117 行)与 varenv。
- **stdio 交错**:分段 stderr 写(每格式片一次 syscall)经 WSL-interop 双管道
  中继与 stdout 交错(SA4 DBG xcw 教训——插桩必须单次原子写或避免)。
- **/tmp 映射**:rubash /tmp → Windows temp,与 WSL /tmp 不同;共享探针文件
  一律走 /mnt/d 挂载路径。

### 现行并行轨道(文件领地不相交)

| 轨道 | 内容 | 预期收割 | 文件领地 |
|---|---|---|---|
| A | globstar 专项(三根因) | ~512 | src/executor/glob.rs 独占 |
| B | fd 模型族(redir fd 生命周期 + procsub /dev/fd/N 抽象) | ~615 | execution_misc/redir 路径 |
| C | 族H 深层展开(new-exp + more-exp IFS 拆分/嵌套) | ~565 | parameter_words/read_split |
| D | iquote/quote lane(\x17 赋值往返 → dquote 内引号标记 → eval RHS 解码) | ~230 | quotes/embedded_parameters(captain 自修) |
| E | ifs-posix LLDB 专项(状态污染,孤立全过全量才崩) | ~1503 | 独占慢跑 |

### 2026-09-03 轨道进展

- **D lane 第一批落地(792aab1c)**:赋值存储的转义引号数据标记提升
  (`x=a\'b` 现存 `a'b`,declare -p `$'a\'b'` 序列化与 GNU 一致)+ walker
  GNU 词扫描分支(词外 `\'` 产数据引号,词内 `\"` 为 Escape、`\'` 双字保真)。
  探针成对验证;8 家族爆炸半径 A/B 全部与全量基线逐项相等,零回归。
- **诚实残余(已插桩定位)**:未加引号词中的 `\"` 在 **tokenizer 层**即被吃
  (`echo a\"b` 得 `ab`,GNU `a"b`;`x=a\"b` 存 `ab`)。修复需专用逃逸双引号
  标记——`\x18` 已过载(walker 解作双引号,conditional/pattern 解作反斜杠),
  与 iquote lane 的标记重设计合并做。
- **busybox ash 第二门禁上线(682 文件,vendor + runner)**:TOTAL=335
  PASS=211 DIFF=117 TIMEOUT=7 SKIP=0(基线 20260903T084103Z;全部超时为
  rubash 侧信号/循环投递;历史 P0 heredoc_huge 未复现,561ms 完成,内容哈希
  一致)。最大真实簇 = 双引号内嵌单引号打穿 quote 扫描器(估 25-35 项),
  与 D lane 同根因。复跑:`wsl bash scripts/run-busybox-ash-difftest.sh`。
