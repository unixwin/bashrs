# Issue Suite DIFF Analysis

> Date: 2026-08-12
> Scope: issue #20-#26 compatibility suites, local reruns, and implementation ownership.

This document is the durable version of the issue-suite run notes. Files under
`target/issue-suites/results/` are raw run artifacts; this document is the
tracked summary used to decide what to fix and where.

The concrete implementation playbook for future agents is
[`docs/gnu-bash-compatibility-implementation-plan.md`](gnu-bash-compatibility-implementation-plan.md).

## Continuation Checkpoint (2026-08-14)

This is the current durable handoff. The worktree was clean at the checkpoint:
`master` and `origin/master` both pointed to `55c5daf`, with Bash submodule
`b4608166` (`bash-5.3-16`). No source or test change was made during the
read-only status review; the next code change must begin from the native Bash
`redir`/`vredir` slice below.

### Evidence Rules

- Raw command output belongs under `target/issue-suites/results/`.
- This document records interpretation, root cause, and durable counts.
- The newest dated entry wins when this document, an older summary, and a
  remote Issue title disagree.
- `.right` runner success is not proof of current Bash behavior; compare the
  official `.tests` body against GNU Bash actual output as well.
- A suite result is not complete evidence until its runner setup, exit code,
  stdout, stderr, and timeout behavior are recorded.

### Active Native Bash Slice: `vredir`

Raw evidence is under
`target/issue-suites/results/native-bash-20260814-vredir/`.

| Test | Current observation | Next action |
|---|---|---|
| `vredir4.sub` | Dynamic fd plus closed fd, nameref, and invalid descriptor behavior differs | Split into one operation per probe; trace virtual fd lifetime |
| `vredir5.sub` | Ordered input redirect followed by heredoc fails in Rubash | Verify left-to-right redirect application and heredoc precedence |
| `vredir6.sub` | Current minimal sequence matches (`ok 1`, then `10`) | Keep as regression; do not change `ulimit` speculatively |
| `vredir7.sub` | Array dynamic fd variant fails | Recheck array element expansion and dynamic-fd close state |
| `vredir8.sub` | `varredir_close`/closed dynamic output diagnostics differ | Trace descriptor ownership and close timing |

GNU reference: `third_party/bash/redir.c` and
`third_party/bash/tests/vredir*.sub`. Rubash owners:
`src/executor/redirection.rs`, `src/executor/trap_exec.rs`,
`src/executor/read_io.rs`, `src/executor/read_redirected_fd.rs`,
`src/executor/read_builtin.rs`, `src/executor/external_setup.rs`, and
`src/executor/types.rs`.

The working hypothesis is a virtual fd state/lifetime and ordered-redirection
problem. It must be confirmed with a minimal reproducer before editing; do not
fix these failures by changing expected output or by adding another upstream
script bridge.

### Next Turn Contract

1. Establish Bash and Rubash output for each primitive in `vredir5`, `vredir7`,
   and `vredir8` separately.
2. Read the matching GNU `redir.c` paths for allocation, `F_DUPFD`, close,
   undo, and `varredir_close`.
3. Implement one root-cause fix in the fd/redirection owner and add one Rust
   regression in `tests/executor_command_chaining/`.
4. Run the focused Rust test, then bounded `run-redir` and `run-vredir`.
5. Append a dated result entry here containing command, status, artifacts,
   root-cause conclusion, and remaining failures.
6. Check for stuck `rubash.exe`, `bash.exe`, suite, and Cargo processes before
   ending the turn.

This contract is intentionally narrower than the final compatibility goal,
but it does not redefine that goal: it is the next verified root-cause step.

## 2026-08-14 Semantic Kernel Migration Slice

The semantic map v2 and first kernel owners are now present. The canonical map
is `docs/semantic-ownership.tsv`, checked by
`scripts/validate-semantic-map.sh`; the former file-by-file inventory remains
provenance only. Unreferenced placeholder module files were removed, while
`upstream_scripts` was preserved.

The follow-up placeholder audit found 265 additional comment-only Rust files
outside the active module tree. It classified 58 as duplicate-owner candidates
and 207 as host/deferred. No file was deleted in that audit; the raw report is
`target/rust-placeholder-audit.tsv` and the disposition rule is documented in
`docs/bash-source-map.md`.

`src/executor/fd_table.rs` now owns shell-visible read/write capabilities,
dynamic slot allocation, dup/move/close state, shared text input offsets, and
child materialization. `Executor` keeps the old environment keys as a
compatibility mirror. `src/shell/state.rs` and `src/shell/variables.rs` add
typed shell state and export serialization, and `src/jobs/table.rs` owns job
identity, jobspec resolution, completion retention, and coprocess endpoint
metadata. Background/coprocess registration and completion marking now feed
that table.

Evidence from the bounded verification turn:

| Command | Result | Raw evidence |
|---|---|---|
| `cargo check` | pass | command output |
| `cargo test --lib fd_table` | 3/3 pass | command output |
| `cargo test --test executor_tests command_chaining::part_080::test_exec_dynamic_` | 12/12 pass | command output |
| `cargo test --test executor_tests command_chaining::part_080` | 149/149 pass | command output; ordered stderr copy regression closed |
| `cargo test --lib` | 169/170 pass | command output; existing Windows bare-drive path assertion |
| `run-redir` | 1/1 pass | `target/bash-upstream-tests/logs/run-redir.log` |
| `run-vredir` | 1/1 pass | `target/bash-upstream-tests/logs/run-vredir.log` |
| `run-coproc`, `run-procsub`, `run-jobs` | 1/1 each | corresponding `target/bash-upstream-tests/logs/` files |

The ordered stderr-copy failures are closed by correcting the parser's
internal inherited-stderr redirect: a path inherited from `2>&1` is represented
as an append redirect rather than as a duplicate-fd redirect. Ordered output
state now reads `FdTable` first, with the environment keys retained only as a
compatibility fallback. The next fd gate is external child materialization and
removal of that fallback mirror; `<>` and the native `vredir4/5/7/8` probes
remain separate gates.

## 2026-08-14 Ordered Output Migration

The current bounded verification is:

| Command | Result |
|---|---|
| `cargo test --test executor_tests command_chaining::part_080 -- --nocapture` | 149/149 |
| `BASH_RUNNER=D:/Git/bin/bash.exe D:/Git/bin/bash.exe scripts/run-bash-upstream-tests.sh run-redir` | 1/1 |
| `BASH_RUNNER=D:/Git/bin/bash.exe D:/Git/bin/bash.exe scripts/run-bash-upstream-tests.sh run-vredir` | 1/1 |

The Rust owner changes are in `src/parser/redirect_assign.rs` and
`src/executor/redirection.rs`. The parser keeps the original `2>&1` ordered
redirect but uses a concrete append node when propagating the already-resolved
stdout path into a compound command. `command_output_fd_state` now starts from
`FdTable` capabilities and only consults legacy fd environment keys for
unmigrated state.

## 2026-08-14 Dynamic FD Move Verification

The active `redir`/`vredir` slice now has a confirmed root cause and a focused
semantic fix. `exec {copy}<&$source` was being treated as an external-command
redirect before the `exec` builtin ran. `command_with_process_substitution_files`
materialized the source virtual fd and advanced its offset to EOF. This made a
following `exec {moved}<&$source-` copy an empty remainder even though Bash
duplicates the source input and only then closes the source.

The fix is in the command execution/fd owner boundary:

- `src/executor/command_execute.rs` keeps dynamic `exec {name}...` on the
  shell-owned path.
- `src/executor/execution_misc.rs` recognizes `&N-` only through the dedicated
  move-aware helper; ordinary redirect classification still accepts only `&N`.
- `src/executor/trap_exec.rs` copies only the input/output side selected by the
  operator, closes move sources, preserves closed dynamic variable values, and
  allows the freed slot to be reused.
- `tests/executor_command_chaining/part_080.rs` covers input-only state and
  input move/close/reuse behavior.

Evidence:

| Command | Result | Raw evidence |
|---|---|---|
| `cargo test --test executor_tests command_chaining::part_080::test_exec_dynamic_ -- --nocapture` | 12/12 passed | Cargo output; generated test files are cleaned by the test |
| `cargo test --test executor_tests command_chaining::part_080 -- --nocapture` | 146/149 passed | Cargo output; remaining 3 are ordered stderr/`<>` failures |
| `BASH_RUNNER=D:/Git/bin/bash.exe D:/Git/bin/bash.exe scripts/run-bash-upstream-tests.sh run-redir` | 1/1, exit 0 | `target/bash-upstream-tests/logs/run-redir.log` |
| `BASH_RUNNER=D:/Git/bin/bash.exe D:/Git/bin/bash.exe scripts/run-bash-upstream-tests.sh run-vredir` | 1/1, exit 0 | `target/bash-upstream-tests/logs/run-vredir.log` and latest `results.tsv` |

The runner's `target/bash-upstream-tests/results.tsv` is overwritten by the
next focused invocation; the two log files are the durable raw evidence for
the two commands. The broader native probe artifacts remain in
`target/issue-suites/results/native-bash-20260814-vredir/`.

The closed-fd diagnostic probe also established the current Bash-version
boundary: GNU Bash 5.2 prints `read error: 10: Bad file descriptor`, while
Rubash prints `read: 10: invalid file descriptor: Bad file descriptor`. The
Rust regression now asserts the shared Bash error class rather than the old
Rubash-specific `invalid file descriptor specification` text.

Remaining work is explicitly separate: `part_080` still has three failures in
ordered stderr copies and read-write `<>` handling. `vredir4/5/7/8` still need
primitive-level comparison even though the checked-in `.right` runner reports
1/1. No issue is closed by this slice.

## Executive Summary

### 2026-08-14 ambiguous redirect after unquoted expansion

Unquoted parameter expansion in a redirection target now follows Bash's
ambiguous-redirect rule. For example, `target="a b"; echo hi > $target`
previously created a file literally named `a b`; it now reports
`a b: ambiguous redirect`, returns status 1, and does not create the file.
The check runs at the common materialized-command boundary so shell builtins,
external commands, and compound commands share the same behavior. Quoted
targets such as `> "$target"` remain valid.

The same validation now rejects invalid expanded fd operands such as
`fd=-1; exec <&$fd`, reporting `-1: ambiguous redirect` instead of trying to
open `&-1` as a filesystem path. This closes two concrete `redir.tests` fd
diagnostic cases in the shared redirection boundary.

Verification: `cargo test --test cli_tests fd_redirects` (13 passed),
`cargo test --test cli_tests -- --nocapture` (150 passed), and the Bash
`run-redir` slice (1/1).

### 2026-08-13 wait -n completed-status retention

`wait -n` now retains the exit status it reaps so a later explicit `wait PID`
can query that completed child as Bash permits. The status is consumed by the
explicit wait, while ordinary explicit waits still remove the job immediately.
This fixes the jobs/wait root-cause slice tracked by Issues #22 and #23.

Verification: `cargo test --test executor_tests command_chaining::part_036`
(27 passed).

### 2026-08-13 command-substitution heredoc collection

Batch stdin execution now waits for command-substitution syntax to close after
all declared heredoc bodies have been collected. Previously parentheses in a
heredoc body could make the top-level input check report an EOF syntax error
before the body and closing `)` were available. Command-list substitution also
now uses the real parser for heredoc-bearing sources.

Verification: `cargo test --test executor_tests command_chaining::part_005`
(40 passed); Bash upstream `run-heredoc`, `run-comsub`, `run-comsub-eof`,
`run-comsub-posix`, `run-redir`, `run-vredir`, and `run-procsub` (7/7).

### 2026-08-13 shopt -o display mode

`shopt -o option` uses the normal readable `name<tab>on|off` format. Rubash
was incorrectly passing the reusable-print flag unconditionally, producing
`set +/-o name` for the non-`-p` form. The `-p` form remains reusable.

Verification: Bash upstream `run-shopt` (1/1) and `cargo test --lib`
(163 passed).

### 2026-08-13 parameter replacement literal backslashes

Parameter replacement patterns containing an escaped backslash now normalize
the lexer quote markers before matching. This makes `${P//\\\\//}` replace
Windows path separators with `/`, matching Bash, while preserving ordinary
glob pattern semantics.

Verification: `cargo test --test executor_tests command_chaining::part_024`
(13 passed); direct Bash/Rubash probe outputs `C:/work/dir/file.txt`.

### 2026-08-13 Arithmetic Literal Diagnostics Progress

Arithmetic literal failures now preserve the GNU Bash `expr.c::strlong`
diagnostic categories instead of collapsing every invalid `base#digits` form
into `value too great for base`. Rubash now distinguishes invalid bases,
missing constants, out-of-range digits, invalid zero/duplicate-base numbers,
and invalid octal constants. A focused probe for `3425#56`, `2#`, `2#44`,
`0#4`, and `2#110#11` matches GNU Bash output and status.

This is a diagnostic/root-cause slice only. The official `arith.tests` and
`arith-for.tests` artifacts remain red because array/associative-array
semantics, conditional assignment precedence, recursive arithmetic expansion,
and arithmetic-for command-substitution parsing still differ.

### 2026-08-13 Arithmetic Increment Expansion Progress

The focused GNU `arith2.sub` slice now matches Bash completely (stdout, stderr,
and exit status). The root cause was the distinction in GNU `expr.c::exp0` and
`evalexp` between an arithmetic expansion's numeric result and its validity:
`$((--7))`, `$((++ 7))`, and related non-lvalue prefix forms preserve the
operand value in expansion context, while `(( -- ))` and `(( ++ ))` remain
command errors. Rubash previously represented both cases as `Option<i128>`,
dropping the expansion result and producing only 14 of the expected 26 lines.
The evaluator now falls back to the operand for the expansion form and keeps
the command-context diagnostic, including Bash's error-token spacing.

Raw verification artifacts are under
`target/issue-suites/results/arith-focus/current2-final2-rubash.*` alongside
the Bash baseline. The complete arithmetic suites are still not green; array
subscript quote handling and other `arith10.sub` contexts remain separate
work.

### 2026-08-13 Pipeline/Fd Progress

The first Windows pipeline lifecycle slice is now covered by real execution
semantics. Rubash starts eligible plain external pipelines concurrently with
OS pipes, waits for the final consumer, then collects upstream process status
with a short natural-exit window before the Windows hard-kill fallback. This
matches the relevant GNU Bash `execute_cmd.c`/`jobs.c` ordering: close unused
pipe ends, wait for the pipeline job, and publish all member statuses to
`PIPESTATUS` before applying `pipefail`.

The corresponding WinuxCmd `head` implementation reads stdin incrementally,
so an unbounded producer can stop after the requested records. Verified
examples include `yes | head -n 3`, a three-stage external pipeline,
`PIPESTATUS`, `pipefail`, and `|&`. The Windows fallback may report status 1
for a producer that must be force-killed; Windows has no SIGPIPE status 141.
This is an environment-specific status difference, while output, final status,
pipeline completion, and process cleanup are covered by regressions.

Focused verification: `cargo test --lib` 153/153; pipefail executor slice
3/3; fd redirect slice 11/11; CLI pipeline/fd regressions passed. Raw suite
artifacts are not generated by this focused change.

### 2026-08-13 Heredoc Progress

Heredoc delivery now follows Bash's quoted-newline rule: unquoted heredocs
remove an unescaped `\\` plus newline before parameter/command expansion,
while quoted heredocs preserve the body literally. On Windows, eligible plain
external pipelines are spawned completely before the first-stage heredoc is
written. This prevents a large heredoc from filling a pipe before downstream
consumers exist. The concurrent path also permits a final `>`/`>>` redirect;
other pipeline redirects remain on the conservative shell-aware path.

Verified focused behavior includes quoted/unquoted heredocs, `<<-`, command
substitution, an external `cat <<HERE | md5sum`, and BusyBox's
`heredoc_huge.tests`. The latter completed with status 0, matching md5 output,
and `End`, with no stderr. The 120KB pipeline-output regression and heredoc
parser/redirection slices also pass. GNU Bash's large-heredoc tempfile fallback
remains a future optimization; the current fix establishes equivalent
completion semantics for the covered Windows pipeline shape.

The failures are not one-off test noise. They cluster into a small number of
Bash semantic subsystems:

### 2026-08-13 Parser Strictness Progress

The coproc actual-output slice was rechecked against GNU Bash and currently
matches, so no speculative coproc rewrite was made. The next ISSUE-backed
parser gap was `[[ -n & ]]` (and equivalent invalid conditional operators):
Bash reports a syntax error and exits 2, while Rubash previously treated the
unknown expression as an ordinary false condition. The parser now marks
invalid conditional separators and unmatched delimiters for the existing
syntax-error path. Focused parser conditionals and CLI regressions pass;
valid logical conditions and pattern RHS expressions remain accepted.

The follow-up parser slice also now rejects unterminated `[[ ...` commands
with status 2 instead of treating `[[` as an ordinary command. Recovery
consumes input through `]]` when present, or to EOF otherwise, preserving the
parser's ability to continue after a malformed conditional in a script.

Command substitution EOF handling is now covered at the lexer/executor
boundary as well: a genuinely missing `)` in `$(...)` returns status 2 and
does not dispatch the partial body as a command. Closed and nested command
substitutions remain green. A substitution whose outer `)` exists but whose
inner `if`/compound command is malformed is also rejected now: compound
sources are routed through the complete command-list parser, and status 2 is
propagated through the outer command. Closed compound substitutions remain
executable, matching the `subst.c` complete-list path and `parse.y` error
handling.

The case-command slice now rejects an invalid top-level compound terminator
inside a case clause, such as `case x in x) done ;; esac`. Bash treats this as
a syntax error rather than a command in the clause. Case parser coverage
remains green, including nested case bodies and command substitutions.

1. **Redirection / fd / Windows device mapping**: `/dev/null`, `2>&1`, `&1`,
   fd duplication/close, ambiguous redirect diagnostics, coproc fd lifetime.
2. **Heredoc**: delimiter collection, heredoc inside command substitution,
   unterminated heredoc diagnostics, and large heredoc buffering/hang behavior.
3. **Parser strictness**: Bash rejects invalid syntax with rc=2 while rubash
   often accepts and exits 0.
4. **Arithmetic status and diagnostics**: invalid bases, division by zero,
   operand errors, and arithmetic-for syntax do not consistently propagate.
5. **Alias / hash / command lookup**: alias expansion timing, quoted alias
   handling, `alias -p`, tracked aliases, hash-table behavior.
6. **Word expansion**: IFS splitting, quoted arrays, substring/slice, pattern
   substitution, RHS expansion, and command substitution state isolation.
7. **Jobs / coproc / process substitution**: job table semantics, wait/kill
   interactions, fd lifecycle, and timeout/hang behavior.
8. **Builtin parity**: option parsing, output, and exit status for `read`,
   `mapfile`, `shopt`, `trap`, `type`, `printf`, `complete`, `getopts`,
   `umask`, `set`, `cd`, `jobs`, and related builtins.

The direction is therefore clear: fix by root-cause subsystem, not by issue
number or by one-off expected-output patches.

## Architecture Boundary

Rubash is being used as the Bash grammar/execution library for Winuxsh, not as
a thin standalone `bash.exe` clone. The ownership boundary should be:

| Layer | Ownership |
|---|---|
| `rubash` | Bash syntax, AST, expansion, heredoc, redirection semantics, builtin behavior, job/trap semantics |
| `winuxcmd` | Windows-native process control, handle/fd backend, pipe/device primitives, hard-kill primitives |
| `winuxsh` | Host shell, interactive/session/profile wrapper, integration that embeds rubash as a library |

Concrete implication: `&1`, `2>&1`, `/dev/null`, coproc fd handling, and Bash
`kill` semantics should be modeled in rubash, backed by winuxcmd primitives
where Windows process/handle operations are needed. They should not be fixed as
Winuxsh UI behavior.

## Suites Covered

The issue corpus includes more than the few suites rerun in the latest session.
The current compatibility picture comes from these sources:

| Suite | Purpose | Current known result | Artifact / source |
|---|---|---:|---|
| Rust unit/lib | Local implementation regressions | 153/153 pass | `cargo test --lib` |
| Rust kill regressions | Windows kill and shell PID handling | pass after `47924f4` | focused `cli_tests` / `executor_tests` |
| Self difftest cases | Hand-authored Bash vs rubash probes | previously 26-case baseline | `tests/difftest/` |
| Bash upstream `.right` runner | GNU Bash `run-*` against checked-in `.right` expectations | 86/87 pass; `run-minimal` has exit-0 log noise | `target/bash-upstream-tests/results.tsv` |
| Bash actual-output | GNU Bash `.tests` bodies, Bash 5.2 actual output vs rubash | 83 files: 15 PASS / 68 DIFF | `target/issue-suites/results/bash-actual/results.tsv` |
| Oil spec | Broad POSIX/Bash/YSH shell behavior corpus | 222 spec files processed; 12 clean, 210 with diffs/failures | `target/issue-suites/results/oils-spec.status` |
| mksh `check.t` | Large shell semantic corpus, Bash-compatible subset used as signal | passed 162; failed 417, including 5 ignored | `target/issue-suites/results/mksh-check-timeout.log` |
| Busybox `ash_test` | POSIX/ash shell behavior, strong heredoc/redir/signal signal | official runner completed; 5 failing scripts in latest run | `target/issue-suites/results/busybox-run-all.log` |
| ksh93 diff | ksh93 regression files, Bash-compatible subset used as signal | 56 files: 25 PASS / 31 DIFF | `target/issue-suites/results/ksh93-diff/results.tsv` |

Notes:

- The Bash upstream `.right` runner is useful but incomplete as a truth source:
  it can pass while Bash 5.2 actual-output still differs.
- ksh93 and mksh include syntax that Bash does not support. Those failures are
  not automatically rubash bugs; only Bash-compatible subsets are repair targets.
- `target/issue-suites` contains temporary runners and logs. It is intentionally
  not the permanent project documentation.

## Bash Actual-Output Result Shape

Latest local rerun:

```text
TOTAL=83 PASS=15 DIFF=68
```

Exit-code shape:

| bash_rc/rubash_rc | Count | Interpretation |
|---|---:|---|
| `0/0` | 52 | Both complete; output/diagnostics differ. These are real semantic diffs. |
| `2/0` | 11 | Bash rejects syntax or builtin usage; rubash silently accepts. |
| `127/0` | 12 | Harness/environment or command lookup mismatch; review before classifying. |
| `124/0` | 5 | Timeout/hang or replicated-runner noise; review individually. |
| `1/0` | 2 | rubash drops runtime/arithmetic failure status. |
| `9/0` | 1 | Termination/status mismatch. |

High-signal Bash actual-output families:

| Family | Representative files | Likely implementation area |
|---|---|---|
| Syntax error acceptance | `parser`, `cond`, `errors`, `glob-bracket`, `arith-for`, `array` | `src/parser/*`, conditional/arithmetic/array grammar, parse error propagation |
| Arithmetic diagnostics/status | `arith`, `arith-for`, `cond`, `quotearray`, `new-exp` | `src/executor/arithmetic/*`, `src/executor/arithmetic_aliases.rs` |
| Heredoc / command substitution | `heredoc`, `comsub-eof`, `comsub-posix`, `exportfunc` | `src/lexer/heredoc*.rs`, `src/parser/command_substitution.rs`, `src/executor/command_substitution*.rs` |
| Redirection / fd semantics | `redir`, `vredir`, `coproc`, `procsub`, `read` | `src/executor/redirection.rs`, `src/executor/external_redirects.rs`, `src/executor/builtin_redirects.rs`, `src/sys/sh/zmapfd.rs` |
| Alias/hash semantics | `alias`, `errors`, `history` | `src/builtins/alias.rs`, `src/builtins/hash.rs`, `src/executor/alias_*.rs` |
| Word splitting / quoting / arrays | `ifs`, `nquote*`, `quotearray`, `rhs-exp`, `assoc`, `array` | `src/executor/expand_word.rs`, `src/executor/parameter_words.rs`, `src/shell/arrays/*` |
| Glob/extglob/brace | `glob`, `globstar`, `extglob`, `braces` | `src/executor/glob.rs`, `src/parser/extglob_pattern.rs`, `src/expand/braces.rs` |
| Builtin option coverage | `complete`, `getopts`, `mapfile`, `read`, `shopt`, `trap`, `type`, `test`, `printf` | `src/builtins/*` |
| Jobs/coproc/process substitution | `jobs`, `coproc`, `procsub`, `varenv` | `src/jobs/*`, `src/parser/coproc_command.rs`, `src/parser/process_substitution.rs`, executor pipe/fd handling |

Concrete redirection/device evidence:

- `vredir6.sub`: `/dev/null: Invalid argument`
- `redir.tests` / `vredir*.sub`: `$fd: Bad file descriptor`,
  `$fd: ambiguous redirect`, `-1: ambiguous redirect`
- `coproc.tests`: coproc descriptors trigger `Bad file descriptor`

These point to an incomplete fd/device model, not just missing output strings.

## ksh93 Diff Result Shape

Latest local rerun:

```text
TOTAL=56 PASS=25 DIFF=31
```

High-signal files where rubash has sharply more failures than Bash:

| File | Bash errors | Rubash errors | Likely implementation area |
|---|---:|---:|---|
| `arith.sh` | 2 | 127 | arithmetic parser/evaluator; ignore ksh-only enum noise |
| `attributes.sh` | 2 | 125 | `typeset`/`declare` attributes and assignment attributes |
| `case.sh` | 7 | 40 | case pattern matching and parser/runtime dispatch |
| `heredoc.sh` | 24 | 127 | heredoc parser/lexer/runtime boundary |
| `jobs.sh` | 2 | 10 | jobs/wait/kill job-control semantics |
| `path.sh` | 1 | 103 | path search, command lookup, script execution paths |
| `quoting.sh` | 51 | 125 | quoting and word expansion |
| `quoting2.sh` | 2 | 65 | quoting and word expansion |
| `statics.sh` | 2 | 127 | shell variable state, readonly, scope semantics |
| `subshell.sh` | 2 | 127 | subshell state isolation |
| `substring.sh` | 2 | 127 | parameter expansion substring/slice |
| `timetype.sh` | 2 | 16 | `time` and typed-variable semantics |
| `vartree1.sh` | 2 | 5 | variable-tree and compound-variable noise |

Classification rule:

- ksh-only constructs such as `enum`, compound variables, `.sh.*`, and typed
  attributes are not Bash compatibility requirements.
- Bash-compatible constructs inside these files still matter, especially
  heredoc, quoting, case, arithmetic, path lookup, jobs, and substring behavior.

## Busybox / mksh / Oil Signals

These suites reinforce the same root-cause groups:

| Suite | Strongest signal | Root-cause family |
|---|---|---|
| Busybox ash | heredoc_huge, redir, signal, vars, parsing | heredoc, fd/device, parser strictness, signal/job semantics |
| mksh | heredoc, integer-base, alias, arrays, command substitution, variable expansion | heredoc, arithmetic, alias, arrays, word expansion |
| Oil spec | word-split, trap/umask/kill/set/cd/echo, redirects, var-op, parse-errors, command-sub | word expansion, builtin parity, redirection, parser strictness, command substitution |

The suites disagree in surface syntax but agree on implementation gaps.

## What Is Missing vs. What Is Merely Incomplete

| Area | Status | Direction |
|---|---|---|
| fd/device model | Partially implemented, incomplete on Windows devices and fd duplication | Build a central Bash fd abstraction backed by winuxcmd handles; normalize `/dev/null`; implement dup/close/error semantics before patching individual tests |
| heredoc | Implemented but semantically incomplete and performance-risky | Fix lexer collection and runtime delivery; add large heredoc regression and command-substitution heredoc cases |
| parser errors | Implemented permissively in places | Add strict Bash parse errors and rc=2 propagation for invalid conditionals, arrays, extglob, arithmetic-for, and substitutions |
| arithmetic | Implemented but error propagation incomplete | Make invalid constants/base/division/syntax produce Bash-compatible diagnostics and statuses |
| alias/hash | Implemented but missing timing/listing/tracked-alias behavior | Align alias expansion timing and builtin output/status; decide whether tracked alias is needed or explicitly scoped |
| word expansion | Broadly implemented but many edge cases remain | Fix IFS empty fields, quoted arrays, substring/slice, patsub, RHS and command substitution state isolation |
| jobs/coproc/process substitution | Partially implemented; fd lifecycle incomplete | Define job table and coproc fd ownership; connect wait/kill/signal behavior to winuxcmd backend |
| builtins | Many exist but option/error parity is uneven | Finish builtin-by-builtin parity using GNU Bash `.def` behavior and issue suite examples |

## Repair Order

1. **fd/device/redirection**: `/dev/null`, `2>&1`, `&1`, fd duplicate/close,
   ambiguous redirect, bad fd diagnostics.
2. **heredoc**: ordinary, command-substitution, unterminated, and huge heredoc.
3. **parser strictness**: rc=2 cases that rubash currently accepts.
4. **arithmetic error propagation**: invalid base, division by zero, syntax.
5. **jobs/coproc/process substitution**: fd lifecycle and wait/kill behavior.
6. **word expansion**: IFS, quoting, arrays, substring/slice, patsub.
7. **builtin parity**: trap/umask/kill/set/shopt/read/mapfile/type/printf/etc.
8. **alias/hash**: expansion timing, listing, tracked alias/hash table behavior.

This order prioritizes correctness foundations that many suites share. Fixes
should be validated with focused Rust tests first, then the smallest relevant
suite slice, then the broader issue-suite family.

### 2026-08-13 Arithmetic Checkpoint

Compared with GNU Bash `execute_cmd.c`/`expr.c`, arithmetic-for now preserves
the failure status for invalid initialization, test, and update expressions.
The previous `!ran_body` cleanup path could overwrite initialization/test
errors with status 0. The executor now tracks arithmetic failure separately,
reports the arithmetic diagnostic, and returns status 1 while retaining the
normal zero-iteration status for a valid false test.

Focused validation:

```text
cargo test --test cli_tests arithmetic -- --nocapture         PASS (3/3)
cargo test --test parser_tests arithmetic_for -- --nocapture PASS (4/4)
cargo check                                                    PASS
```

Remaining arithmetic work is broader expression parity: complete invalid
lvalue diagnostics, `let` compound-expression parsing/status, conditional
arithmetic errors, and the Bash actual-output `arith`/`arith-for` suite slice.

### 2026-08-13 Arithmetic Expansion Follow-up

GNU Bash `expr.c::evalexp` reports an invalid expression through `validp`,
while `subst.c` distinguishes a failed arithmetic expansion in an ordinary
word from a failed assignment expansion. Rubash now follows that boundary:

- lexer continuation scanning treats `$((...))` as arithmetic context, so `#`
  in a base literal is not mistaken for a shell comment;
- ordinary words such as `echo $((2#44)); echo after` fail only the current
  command and allow the following command to run;
- assignment words still abort the current command list on arithmetic failure;
- `let`, `(( ))`, `[[ ]]`, arithmetic-for, and embedded arithmetic expansion
  paths now report a diagnostic even when the exact parser token is unknown.

Focused results are stored under
`target/issue-suites/results/arith-status-*` and
`target/issue-suites/results/arith-base-*`. Rust arithmetic tests remain green
(5/5); the official `arith.tests` and `arith-for.tests` files still have
broader unrelated differences and are not yet classified as passing.

### 2026-08-13 Arithmetic Empty/Subscript Progress

Compared with GNU Bash `expr.c::subexpr` and
`arrayfunc.c::array_value_internal`, Rubash now treats empty arithmetic
expressions as zero and performs quote removal inside indexed arithmetic
subscripts before evaluating the index. `$(( ))`, `$((\"\"))`,
`[[ 0 -eq \"\" ]]`, and `(( a[\" \"]=11 ))` match Bash in focused probes.
The empty-expression and indexed-subscript behaviors are covered by CLI
regressions.

The remaining `arith10.sub` mismatch is narrower: assignment words using
escaped quotes such as `a[\\\" \"]=15` are still split incorrectly by the
lexer/parser before the array-assignment owner receives them. No broad lexer
heuristic was retained after it failed to improve the exact reproducer.

## 2026-08-12 Repair Checkpoint

### 2026-08-13 umask symbolic permission progress

The next builtin-family gap was in `src/builtins/umask.rs`. Compared with
GNU `builtins/umask.def::parse_symbolic_mode`, Rubash accepted only `rwx`
permissions in symbolic modes. Bash also accepts permission-copy operands
(`u`, `g`, `o`) and conditional execute permission (`X`), which are used by
the upstream `builtins8.sub` coverage.

Rubash now maps copied permissions from the selected source class and applies
`X` only when the current allowed mode has an execute bit, while retaining the
shell-local umask state used by the Windows host. Added unit coverage for
`o=u`, `o=g`, and `a+X`.

Verification:

```text
cargo test --lib umask: 3 passed
cargo test --test cli_tests umask: 1 passed
run-builtins: 1/1 passed
```

The follow-up comparison with `umask.def::parse_symbolic_mode` also fixed two
details that the first implementation missed. Permission-copy operands and
`X` now use the mode bits from before the complete clause list, matching
Bash's `initial_bits` argument even after an earlier clause modifies the
working mask. Invalid numeric modes and invalid symbolic modes now preserve
Bash's diagnostic categories (`octal number out of range`, invalid symbolic
operator, or invalid symbolic character) instead of collapsing them into one
message. Five unit tests cover the successful and diagnostic paths.

### 2026-08-13 trap action-only inspection

GNU `builtins/trap.def::trap_builtin` supports `-P` in addition to `-l` and
`-p`. `-P` prints only the stored action for the specified signal, requires at
least one signal name, and cannot be combined with `-p`. Rubash previously
treated `-P` as an invalid signal specification because its option handling
only recognized `-l`.

`src/builtins/trap.rs` now parses the three GNU display options, preserves the
existing reusable `trap -- ACTION SIGNAL` output for `-p`, and emits action-only
output for `-P`. Invalid options and the two `-P` usage errors return status 2.
This is shell-table behavior; native signal delivery remains owned by the
executor/backend boundary.

Verification:

```text
trap-related executor tests: 22 passed
cargo test --test cli_tests trap: 2 passed
run-trap: 1/1 passed
cargo check: passed
```

### 2026-08-13 shopt reusable-query status

GNU `builtins/shopt.def::list_shopts` and `list_shopt_o_options` return failure
when a specified option is disabled, even when `-p` prints its reusable form.
Rubash printed the correct `shopt -u NAME` or `set +o NAME` line but returned
success from those `-p` paths. The executor now preserves the Bash query status
for `shopt -p NAME` and `shopt -o -p NAME`; quiet `-q` behavior remains
unchanged. Added a regression covering disabled and enabled options and the
`set -o` namespace.

Verification:

```text
shopt-related executor tests: 13 passed
cargo test --test cli_tests shopt: 4 passed
run-shopt: 1/1 passed
```

### 2026-08-13 kill process-group probe

GNU `builtins/kill.def` accepts PID `0` as the current process-group target.
The `kill -0 0` form is an existence probe and must succeed when the shell's
process group exists. Rubash's builtin parser rejected zero, and its executor
had a fast path for `kill -0` that called `process_exists(0)` before reaching
the builtin, producing `No such process` on Windows.

Both semantic owners now accept PID zero for signal-zero probes. The Windows
backend is still used for non-zero signals, since `OpenProcess` cannot express
a process-group target; this avoids claiming successful group termination.

Verification:

```text
kill CLI tests: 7 passed
kill executor tests: 20 passed
run-builtins: 1/1 passed
kill -l name/number/128+number probes: matched GNU Bash
```

### 2026-08-13 set invalid `-o` status

GNU `builtins/set.def::set_builtin` reports an invalid `set -o` option name as
`EX_USAGE` (status 2), while a valid option query can return status 1 for a
disabled option. Rubash emitted the same diagnostic but returned status 1 for
both cases. The `set -o` name-validation branch now returns 2, preserving the
distinction between usage errors and ordinary option state failures.

Verification:

```text
set unit tests: 10 passed
set CLI tests: 4 passed
set/executor command tests: 101 passed
run-builtins: 1/1 passed
```

Current local repair work moved the first two repair-order families forward:

- **fd/redirection**: centralized persistent fd redirection handling for
  builtins, external commands, `echo`, `trap`, and `read`; added coverage for
  close/dup behavior including `exec 3>&-`, closed input fds, dynamic fd
  expansion, and mapfile/readarray `-u` cases.
- **coproc fd lifecycle**: close/dup of persistent coproc descriptors now keeps
  `coproc_stdin_writers` and `coproc_stdout_readers` in sync with fd state, so
  closing a coproc stdin fd can unblock the reader and duplicated coproc writer
  fds remain writable.
- **heredoc parser/runtime**: heredoc bodies are assigned through nested
  compound command structures, including brace groups and pipeline nodes, and
  malformed reserved-word cases now produce parse errors instead of executing
  later heredoc text as commands.
- **pipeline boundary**: plain external pipelines can use concurrent OS pipes
  when the first stage has heredoc/here-string input, but rubash no longer
  rewrites external command arguments such as `head -3000`; GNU coreutils
  compatibility for external commands belongs in winuxcmd.

Focused checks passed locally:

```text
cargo test --lib --target-dir target/trace-test
cargo test --test parser_tests heredoc --target-dir target/trace-test -- --nocapture
cargo test --test cli_tests heredoc --target-dir target/trace-test -- --nocapture
cargo test --test cli_tests coproc -- --nocapture
cargo test --test cli_tests c_command_mapfile_ -- --nocapture
```

### 2026-08-14 Pipeline missing-command diagnostics

Pipeline stages now preserve the ordinary external-command failure contract.
When a stage is absent from `PATH`, it returns status 127 and emits
`<command>: command not found`, instead of returning the internal
`pipeline command could not execute: builtin command not found` error. This
keeps missing WinuxCmd wrappers (for example a dispatcher command without a
`head.exe` entry point) distinguishable from genuine builtin pipeline
failures, while existing builtin stages such as `set | cat` and `export | cat`
continue through the shell-aware pipeline path.

Focused verification:

```text
cargo test --test cli_tests pipeline_missing_external_command_reports_command_not_found: pass
cargo test --test cli_tests c_command_reads_named_coproc_stdout_through_array_fd: pass
cargo test --test cli_tests c_command_writes_to_named_coproc_stdin_fd: pass
```

### 2026-08-14 compatibility slice refresh

The bounded Bash comparison slices for `getopts`, `nameref`, `ifs`, `trap`,
`parser`, `redir`, `arith`, and `arith-for` all pass against the current
Rubash executable. The 26 checked-in differential probes also pass, including
the command-substitution, heredoc receiver, case-pattern, path-form,
redirection, parameter-pattern, debug-trap, positional, var-op, arithmetic,
and alias cases.

The remaining P0 report for BusyBox `heredoc_huge` is an external Windows
pipeline integration issue: the reproducer is `yes | head -3000 | md5sum`,
while the current environment exposes the WinuxCmd dispatcher as
`winuxcmd.exe` without individual command entry points. Rubash now detects a
dispatcher-owned command through `winuxcmd help <command>` and passes the
original command name to every external launch path. The current WinuxCmd
`yes` implementation intentionally emits only 1000 lines, so the 3000-line
fixture remains a backend-capacity failure rather than a Rubash lookup or
heredoc-collection failure.

Open follow-up: validate the full `yes | head -3` and `yes | head -3000 |
md5sum` slices after WinuxCmd raises or removes its safety output cap. Rubash's
remaining integration work is then pipe/handle close and upstream-process
waiting validation, rather than command-name argument normalization.

## Validation Rules Going Forward

- Keep raw suite logs under `target/issue-suites/results/`.
- Keep durable interpretation in `docs/`.
- Every compatibility fix should state:
  - the issue number or suite family it moves;
  - the root-cause family above;
  - the specific module(s) changed;
  - the focused test added or updated;
  - the suite slice rerun.
- Do not remove `src/executor/upstream_scripts*` while the issue suites are
  still red; treat it as temporary compatibility scaffolding until replaced by
  real semantics and tests.
- Do not count ksh-only syntax as required Bash support unless GNU Bash can run
  the construct or a Bash-compatible subset is being tested.
### 2026-08-13 arithmetic escaped-quote array assignment

`run-arith` now passes after fixing the lexer boundary for indexed array
assignment words such as `a[\" \"]=15`. Bash keeps the escaped-quote subscript
inside one assignment word; Rubash previously split at the embedded space and
reported `a[\": command not found`. `src/lexer/word.rs` now recognizes the
narrow `name[...]` plus assignment form and preserves whitespace while scanning
the subscript, including escaped quotes and nested brackets. The parser and
executor then receive the complete word and resolve the quoted arithmetic
subscript to indexed-array element `0`.

Regression coverage:

- `src/lexer/tests.rs::test_escaped_quote_array_assignment_stays_one_word`
- `cargo test --test cli_tests arithmetic` (8 passed)
- `cargo test --test parser_tests array_element_assignment` (8 passed)
- `scripts/run-bash-upstream-tests.sh run-arith` (1/1 passed)

Raw runner output: `target/issue-suites/results/arith-focus/run-arith.log`.

### 2026-08-13 coproc reader lifetime

The coprocess actual-output difference exposed a real fd-lifetime bug in
`src/executor/read_io.rs`: `read_coproc_stdout` used `read_to_end`, removed the
pipe reader after the first `read`, and returned only the first logical line.
Unread records were therefore lost on the next `read <&${COPROC[0]}`. GNU Bash
keeps the coprocess descriptor open, as reflected by `execute_cmd.c` and
`redir.c`; Rubash now reads one record or requested character limit at a time
and retains the reader until EOF.

Regression coverage:

- `tests/cli_tests.rs::c_command_keeps_unread_coproc_records_for_later_reads`
- `cargo test --test cli_tests coproc` (6 passed)
- `cargo test --test cli_tests c_command_read_` (4 passed)
- `scripts/run-bash-upstream-tests.sh run-coproc` (1/1 passed)

Raw runner output: `target/issue-suites/results/next-run-coproc-after.log`.

### 2026-08-13 case parser reserved-word boundaries

The Bash actual-output `parser` difference included malformed case forms that
Rubash accepted with status 0: `case x in esac) ... esac` and `case in do do)
... esac`. GNU `parse.y` treats `esac`/`do` at these grammar boundaries as
reserved tokens, not as ordinary case-word or pattern text. `src/parser/case_command.rs`
now rejects those boundaries and lets the parser's existing rc=2 error path
handle them. The check preserves Bash-valid empty cases with trailing
redirections and valid patterns beginning with the literal `esac`.

Regression coverage:

- `tests/cli_tests.rs::malformed_case_reserved_word_boundaries_are_syntax_errors`
- `cargo test --test parser_tests case` (46 passed)
- `cargo test --test cli_tests malformed_pipeline_and_if` (1 passed)
- `scripts/run-bash-upstream-tests.sh run-parser` (1/1 passed)

Raw runner output: `target/issue-suites/results/next-run-parser.log`.

### 2026-08-13 for/select identifier validation

GNU Bash `execute_cmd.c::execute_for_command` and
`execute_select_command` call `check_identifier` during execution. Rubash
previously rejected an invalid loop variable in the parser, discarded the
compound-command AST, and then executed the remaining tokens as ordinary
commands. This produced the wrong command-not-found/syntax behavior. The
parser now preserves the `for`/`select` node; the executor reports
`` `name': not a valid identifier`` and returns 1 in normal mode, or 2 in
non-interactive POSIX mode, matching Bash's `posixly_correct` branch.

Regression coverage:

- `tests/cli_tests.rs::invalid_for_and_select_names_fail_at_execution_like_bash`
- `tests/cli_tests.rs::invalid_for_name_is_fatal_in_posix_mode`
- `cargo test --test parser_tests for` (31 passed)
- `scripts/run-bash-upstream-tests.sh run-parser` (1/1 passed)

Raw runner output: `target/issue-suites/results/next-run-parser-for.log`.

### 2026-08-13 array element assignment diagnostics

The array actual-output difference exposed missing execution-time validation.
GNU `arrayfunc.c::assign_array_element` rejects empty and invalid indexed
subscripts, non-numeric indexed keys, negative indexes that cannot resolve, and
list values assigned to a single array member. GNU `execute_cmd.c` also keeps
readonly assignment handling separate from ordinary identifier validation.
Rubash now performs these checks in `src/executor/array_assignment_exec.rs` and
recognizes `readonly name[subscript]` as Bash's invalid-identifier diagnostic in
`src/builtins/setattr/apply.rs`. The lexer also keeps a parenthesized array
element value atomic so the executor can report the correct list-assignment
error.

Regression coverage:

- `tests/cli_tests.rs::array_element_assignment_reports_bash_subscript_errors`
- `tests/cli_tests.rs::readonly_array_element_argument_matches_bash_identifier_diagnostic`
- `cargo test --test cli_tests arithmetic_array_subscript_quote_removal_targets_index_zero`

Raw suite artifact: `target/issue-suites/results/bash-actual/work/array/`.

### 2026-08-13 loop-control diagnostics outside loops

Oil/mksh `break-5` and `continue-5` exposed that Rubash already emitted
Bash's “only meaningful in a loop” diagnostic but reset the command status to
zero. GNU Bash leaves the command list running while making `$?` equal to 128.
`src/executor/pwd_loop_builtins.rs` now preserves status 128 for both builtins;
loop-internal `break`/`continue` handling is unchanged.

Verification: `cargo test --test executor_tests part_050` (14 passed), plus
direct `rubash -c 'break; echo status:$?'` and `continue` probes.

### 2026-08-13 readonly assignment fatal status

The Oil `bugs` readonly-assignment case exposed a status/lifetime mismatch:
Rubash printed `readonly variable` and set status 1, but continued a plain
assignment command list. GNU Bash treats a direct assignment to a readonly
variable as a fatal non-interactive shell error. The assignment owner now
returns `ExecuteError::ExitCode(1)` for that case, while `declare`/`export`/
`local` keep their builtin-specific recoverable diagnostics.

Verification: direct readonly assignment regression, `part_018` (13 passed),
and `part_019` (10 passed).

### 2026-08-13 refreshed upstream slice audit

The old Bash actual-output snapshot was stale after the recent parser,
arithmetic, array, alias, heredoc, and redirection repairs. Bounded reruns now
pass these GNU Bash slices: `run-arith`, `run-array`, `run-parser`,
`run-heredoc`, `run-redir`, `run-alias`, `run-builtins`, `run-attr`,
`run-trap`, `run-jobs`, and `run-vredir` (each 1/1).

The refreshed actual-output runner still reports 15/83 exact matches, but its
remaining differences include nested test-driver invocations, Windows path
lookup, command availability, and timeout-dependent job/process cases. One
reproducible semantic lead remains: Bash rejects a newline-separated `for`
header inside a `case` clause at `do`, while Rubash accepts it. The lexer
currently represents physical newlines and semicolons with the same token shape;
the next parser pass should preserve line-break provenance before changing this
grammar boundary. No broad rejection rule was added in this pass.

Raw refreshed artifact: `target/issue-suites/results/bash-actual/`.

### 2026-08-13 nested case/for newline grammar

GNU `parse.y` keeps newline-list boundaries significant while parsing a
compound list. Rubash previously collapsed physical newlines and explicit
semicolons into identical `Semicolon` tokens, so a `for` header nested directly
inside a `case` clause could accept `for x` followed by a newline and execute
its body where Bash rejects the construct at `do`.

`Token` now records `line_break` provenance. `case_command.rs` uses that
provenance only for the nested case-clause grammar boundary; top-level
`for x\nin ...` remains valid. The parser returns the existing rc=2 recovery
path and does not execute the malformed clause.

Regression coverage:

- `tests/parser_tests.rs::test_case_rejects_newline_for_header_at_do_boundary`
- `tests/parser_tests.rs::test_top_level_for_still_allows_newline_before_in`
- `tests/cli_tests.rs::newline_for_header_inside_case_is_a_syntax_error`
- `cargo test --test parser_tests for` (33 passed)
- `scripts/run-bash-upstream-tests.sh run-parser` (1/1 passed)

Parser error recovery now reports `do` for this specific malformed nested
`for` header, matching Bash's diagnostic token; other malformed case forms
continue to use their enclosing `esac` recovery token.

### 2026-08-13 set option operands

GNU `builtins/set.def::set_builtin` continues scanning after `-o`/`+o`
option names and assigns the remaining non-option words to positional
parameters. It also treats an empty `+` option word as the start of positional
assignment. Rubash's executor previously returned after these option forms,
leaving `$#`, `$1`, and `$2` unchanged. The executor now uses an indexed scan,
skips the long option name, applies the option state, and assigns remaining
operands through the same positional-parameter path. `posix` state updates are
kept synchronized with the visible environment marker.

Regression coverage:

- `tests/executor_command_chaining/part_058.rs::test_set_plus_assigns_operands_after_empty_option_word`
- `tests/executor_command_chaining/part_058.rs::test_set_long_option_continues_to_positional_operands`
- `tests/executor_command_chaining/part_058.rs::test_set_plus_long_option_continues_to_positional_operands`
- `cargo test --test executor_tests command_chaining::part_058` (16 passed)
- `cargo check` and `cargo fmt --all -- --check`

### 2026-08-13 remove invert upstream-script bridge

`src/executor/upstream_scripts` still contains compatibility scaffolding that
matches a GNU test filename and prints the checked-in `.right` output instead
of executing the script. The `invert.tests` handler was no longer needed:
GNU's cases use ordinary `!` status inversion over simple commands, pipelines,
and subshells, all of which are covered by the parser and executor's real
inversion paths. The handler and its embedded expected-output constant were
removed; the upstream test now executes through normal semantics.

Verification:

- `scripts/run-bash-upstream-tests.sh run-invert` (1/1 passed)
- inversion parser tests (6 passed)
- inversion executor tests (6 passed)
- `cargo check` and `cargo fmt --all`

### 2026-08-13 remove herestr and strip upstream-script bridges

Two more upstream test handlers were removed after checking their GNU test
bodies against the real Rubash owners. `herestr.tests` now exercises lexer
here-string collection, expansion, `read -a/-d`, functions, and fd handling
directly. `strip.tests` now exercises legacy backtick command substitution and
Bash's trailing-newline removal directly. Neither handler needed Windows-only
output normalization or process substitution fixtures, so printing the
checked-in `.right` file was scaffolding rather than an architectural
requirement.

Verification:

- `scripts/run-bash-upstream-tests.sh run-herestr` (1/1 passed)
- `scripts/run-bash-upstream-tests.sh run-strip` (1/1 passed)
- here-string/read focused executor tests (26 passed)
- command-substitution focused tests (54 passed)
- backtick-focused tests (10 passed)
- `cargo check` and `cargo fmt --all -- --check`

### 2026-08-13 remove tilde and tilde2 upstream-script bridges

The `tilde.tests` and `tilde2.tests` handlers were also no longer required.
Both test bodies now execute through the parser, word/assignment expansion,
parameter expansion, and POSIX-mode state owned by Rubash. The `tilde2` slice
was checked in an isolated work directory with its companion `tilde3.sub` and
the GNU `support/recho.c` helper built locally; Bash and Rubash matched on
stdout, stderr, and exit status.

Verification:

- `scripts/run-bash-upstream-tests.sh run-tilde2` (1/1 passed)
- direct `tilde2.tests` Bash/Rubash comparison (stdout/stderr/rc matched)
- `tests/executor_tests.rs::test_quoted_assignment_like_argument_suppresses_tilde_expansion`
- `cargo check`, `cargo fmt --all -- --check`, and `git diff --check`

### 2026-08-13 upstream bridge audit: type and shopt remain semantic gaps

The next candidates were checked by executing renamed copies of the upstream
scripts with their companion files. `type.tests` still differs in function
body reconstruction, heredoc/coprocess pretty-printing, hash accounting, and
unset-`PATH` lookup behavior; its companions also expose parser gaps. The
`shopt.tests` body is mostly aligned, but `shopt1.sub` still differs in the
process-substitution execution of executable helper files without a shebang.
That path must preserve the parent shopt state and Bash's ENOEXEC fallback
semantics. The existing bridges therefore remain in place until those real
owners are fixed and covered.

### 2026-08-14 kill explicit signal-zero process-group operand

GNU Bash treats `kill -s 0 -1` as a signal-zero existence probe against the
current process group. Rubash's option scanner previously continued parsing
after `-s 0`, interpreted `-1` as another signal specification, and returned
usage status 2. The scanner now treats the remaining words as operands after
an explicit `-s`/`-n` option; signal-zero group operands are accepted by the
Windows backend without attempting native process termination.

Verification:

- `cargo test --test executor_tests command_chaining::part_026` (15 passed)
- direct Bash/Rubash probe for `kill -s 0 -1` (both status 0)

### 2026-08-14 default SIGCHLD disposition

The pending-signal dispatcher treated every signal without an installed trap
as a fatal `128 + signal` exit. Bash ignores child-completion notifications
by default, so a queued SIGCHLD could incorrectly terminate a shell after a
background child was reaped. `run_pending_signal_traps` now discards the
default SIGCHLD notification unless a CHLD trap is explicitly installed; other
unhandled signals retain their existing fatal behavior.

Verification: `cargo test --test executor_tests command_chaining::part_045`
(13 passed) and `cargo test --lib` (162 passed).

### 2026-08-14 external argument path conversion boundary

Issue #31's remaining root cause was broader than the original `/h/` special
case: every nonexistent `/X/...` argument was converted to `X:\...`, which
could corrupt regexes and Git pathspecs. External argument conversion now
keeps explicit `/c/...` shell display paths convertible, but preserves other
drive-shaped arguments unless the translated target exists. Added a Windows
regression for `/h/not-a-real-pathspec`.

Verification: `cargo test --lib path` (20 passed) and `cargo test --lib`
(163 passed).

### 2026-08-13 WinuxCmd streaming `head` fixes the producer hang

The BusyBox `heredoc_huge`/large-producer failure was reproduced through the
Windows external pipeline path. WinuxCmd's `head` decoded stdin by first
reading the entire stream, so an unbounded producer such as `yes` could never
reach EOF and the pipeline deadlocked. The stdin path in
`WinuxCmd/src/commands/head.cpp` now uses the existing bounded streaming
reader; whole-stream text decoding remains for regular files. WinuxCmd's
`yes` implementation continues until its downstream pipe closes.

Verification with the rebuilt WinuxCmd backend:

- `cargo test --test cli_tests external_pipeline` (4/4 passed)
- `cargo test --test cli_tests external_pipeline_writes_large_output_redirect_without_blocking`
  (passed; 120,000-byte output)

The implementation change is in the separate WinuxCmd working tree and must
be committed/released there before this evidence can close the Rubash-side
issue.

The same day, the bounded GNU Bash upstream slices were rerun with the local
GNU Bash 5.2 runner at `D:/Git/bin/bash.exe`: `run-parser`, `run-arith`, and
`run-redir`, `run-getopts`, `run-trap`, and `run-coproc` each passed 1/1. These `.right`-based slices are focused regression
evidence; they do not by themselves close the broader Bash actual-output
differences tracked by Issues #20--#25.

### 2026-08-13 process-substitution redirect recovery

The `shopt1.sub` investigation exposed a narrower process-substitution
regression: output process substitution rewrote `redirect_out`, but the ordered
`redirects` list could still retain the original `>(...)` target. Builtins such
as `echo` that honor ordered redirection then tried to open the process
substitution syntax as a Windows path. The materializer now keeps the ordered
redirect list synchronized with rewritten process-substitution temp paths.

The same slice also tightened Windows same-shell script detection so only
path-qualified `.sh` or extensionless text scripts are executed directly by
Rubash; ordinary `PATH` commands and builtin-like helpers are not intercepted by
current-directory files. `cat` now treats structured redirection targets as
redirection metadata, not input file operands, so `cat > file` can consume
virtual stdin from `>(...)`.

Verification:

- `cargo test --test executor_tests command_chaining::part_047::test_output_process_substitution_redirect_feeds_command_stdin`
- `cargo test --test executor_tests command_chaining::part_047::test_process_substitution_extensionless_script_preserves_shopt_state`
- `cargo test --test executor_tests process_substitution` (56 passed)
- `scripts/run-bash-upstream-tests.sh run-shopt` (1/1 passed via existing bridge)
- renamed `shopt.tests` Bash/Rubash comparison still differs, so the `shopt`
  upstream bridge remains in place.

### 2026-08-14 FD table external-child setup slice

The virtual FD table is now preferred when external child setup resolves
persistent input descriptors. Text-backed dynamic fds are materialized from
the table, file-backed read endpoints are passed as their resolved path, and
the old `FD_STDIN_*` environment keys are used only when no table entry exists
or when the inherited/function-stdin adapter is active. External default
stdout/stderr setup also honors table-backed file endpoints and closed
descriptors; stdout/stderr, coproc, and process-substitution adapters remain
in their existing compatibility paths.

This slice was driven by `vredir5.sub`, `vredir7.sub`, and `vredir8.sub`.
It does not claim that the child receives arbitrary numbered Windows handles;
that remains the next materialization gate for coproc and non-text endpoints.

Verification:

- `cargo check` passed; only the known scaffold warnings remain.
- dynamic-fd focused tests: 12/12 passed.
- `cargo test --test executor_tests command_chaining::part_080`: 149/149
  passed before the new regression; the added persistent-stdout regression
  also passed (the slice is now 150 tests).
- `run-redir`: 1/1 passed; raw log:
  `target/bash-upstream-tests/logs/run-redir.log`.
- `run-vredir`: 1/1 passed; raw log:
  `target/bash-upstream-tests/logs/run-vredir.log`.
- semantic map validation, placeholder audit, and `git diff --check` passed.

The upstream runner's `results.tsv` is single-run state and was last written
by `run-vredir`; retain the two log paths above as the durable evidence for
both bounded invocations. The official Bash `.tests` actual-output gap and
the remaining `upstream_scripts` bridge are unchanged.

### 2026-08-14 Coproc external-child materialization

The next FD boundary is now covered for coprocess endpoints. When an external
command uses `cat <&"${NAME[0]}"`, child setup clones the registered coproc
stdout reader from the `FdTable`; when a command uses
`... >&"${NAME[1]}"`, it clones the registered coproc stdin writer. The
compatibility environment descriptors remain an adapter for shell builtins and
legacy paths, but are no longer the source of truth for these child redirects.

Completion, `wait`, `fg`, `kill`, `disown`, and `ulimit` cleanup paths now close
the corresponding virtual fd-table entry together with the Windows pipe maps.
This prevents a completed or detached coprocess PID from remaining a usable
virtual descriptor.

Focused verification:

- `cargo test --test cli_tests c_command_`: 38/38 passed, including external
  coproc stdout and stdin regressions.
- The existing coproc builtin/read regressions remain passing; the external
  tests exercise the child-materialization boundary rather than only the
  builtin pipe adapter.
- `git diff --check`: passed.

The remaining FD gate is ordered redirect parity and arbitrary non-text
endpoints such as process substitution. This slice does not remove the
`upstream_scripts` coproc bridge or claim closure of the official Bash
`.tests` actual-output differences.

### 2026-08-14 Indexed dynamic-fd and ordered heredoc slice

Direct execution of GNU `vredir5.sub` and `vredir7.sub` exposed two real
semantic gaps that the `.right` runner did not isolate. Dynamic-fd names such
as `{fd[0]}` were rejected by the scalar-only name parser and, when accepted
as text, would have been written under the literal `fd[0]` environment key
instead of the indexed-array storage. The executor now validates numeric
indexed dynamic-fd names, updates the existing array storage, and reads the
stored element when closing the fd.

The same probes showed that `read line <&$stdin <<EOF` must use the final
unnumbered heredoc as stdin. `read_input_for_command` now applies that
precedence only when no explicit `read -u N` fd was selected, preserving the
explicit-fd case.

Direct GNU/Rubash observations:

- `vredir5.sub`: both produce `12 10`, two `a` lines, and the `swizzle`
  function listing after this fix.
- `vredir7.sub`: the indexed `{fd[0]}` / `{fd[1]}` form now produces the same
  content and no longer raises a missing-file error.
- `vredir8.sub`: `/dev/tty` is unavailable in the non-interactive Windows test
  host; this remains a host-owned diagnostic boundary, while the subsequent
  closed-fd checks remain covered by the existing focused tests.

Verification:

- `cargo test --test executor_tests command_chaining::part_080`: 152/152.
- `cargo test --test cli_tests c_command_`: 38/38.
- `run-redir` and `run-vredir`: PASS 0; raw logs remain under
  `target/bash-upstream-tests/logs/`.
- `git diff --check`: passed.
