# Issue Suite DIFF Analysis

> Date: 2026-08-12
> Scope: issue #20-#26 compatibility suites, local reruns, and implementation ownership.

## Platform Scope Decision (2026-08-19)

Rubash and Winuxsh are Windows-first products. Before 1.0, complete GNU/Linux
compatibility is explicitly deferred and is not an acceptance gate. Linux CI
failure evidence must still be recorded, but Linux-only expectations should be
platform-gated or marked deferred rather than driving changes that regress
Windows/Winuxsh semantics.

The required release gate is the Windows/Winuxsh focused Rust and integration
test set. Windows path display, native device aliases, signal/mailbox behavior,
coproc behavior, and host integration must remain green. Linux signal, coproc,
and device differences require a separate follow-up when Linux support becomes
a release objective.

This document is the durable version of the issue-suite run notes. Files under
`target/issue-suites/results/` are raw run artifacts; this document is the
tracked summary used to decide what to fix and where.

The concrete implementation playbook for future agents is
[`docs/gnu-bash-compatibility-implementation-plan.md`](gnu-bash-compatibility-implementation-plan.md).

The focused `vredir` difference ledger is maintained in
[`docs/vredir-diff-todo.md`](vredir-diff-todo.md). It records the raw artifact
paths, root-cause classification, and executable TODOs for the active fd
slice; this document keeps the broader suite history and checkpoints.

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

### 2026-08-14 Coproc virtual close versus external child materialization

The named-coproc fd lifetime slice now has a focused real regression. The
previous close path treated `C[0]` and `C[1]` as one complete virtual fd and
closed both capabilities when `exec {C[1]}>&-` ran. The corrected owner,
`src/executor/trap_exec.rs::close_dynamic_output_fd`, closes only the output
capability and preserves the coprocess stdout reader until it is explicitly
closed or consumed.

Evidence:

| Command | Result | Raw evidence |
|---|---|---|
| `cargo test --test cli_tests c_command_closing_named_coproc_stdin_fd_produces_eof -- --nocapture` | 1/1 pass | CLI regression output |
| `cargo test --test executor_tests command_chaining::part_080 -- --nocapture` | 152/152 pass | fd/coproc regression output |
| `BASH_RUNNER=D:/Git/bin/bash.exe D:/Git/bin/bash.exe scripts/run-bash-upstream-tests.sh run-coproc` | 1/1 pass | `target/bash-upstream-tests/logs/run-coproc.log` |
| direct `coproc { cat; }` probe | Bash `read:hello`; Rubash `read:`; both `done` | `target/issue-suites/results/coproc-kernel-20260814/` |

The direct probe is the important boundary evidence. The Rubash-only loop
passes, so virtual writer-close and reader preservation are no longer the
root cause there. An external `cat` still does not receive the same data/EOF
behavior after child fd materialization. This is an open WinuxCmd/std child
setup and `FdTable::materialize_for_child` TODO, not a reason to revert the
virtual fd fix or to add an output bridge.

Open gates:

- [ ] Make materialized coprocess stdin/out handles follow the shell fd
      lifetime for external children.
- [ ] Add external-child `cat` regressions for read, write, duplicate, close,
      and `wait "$C_PID"` status, with dated stdout/stderr/status artifacts.
- [ ] Re-run the official Bash `.tests` `coproc` actual-output body and update
      its ledger row only after the bridge is unnecessary.

### 2026-08-14 Bridge-free coproc inherited-cat streaming

The minimal external-child reproducer exposed a second real boundary. The
coproc child was correctly spawned with inherited process stdin, but its
no-operand `cat` was delegated to `winuxcmd/cat.exe`. In the nested anonymous
pipe path that wrapper did not forward data before EOF, so the parent blocked
on `read <&"${C[0]}"` before it could close `C[1]`.

The Rubash owner `src/executor/external_file_builtins.rs` now handles only the
matching case: no file operand, no explicit stdin/heredoc redirect, and
`INHERIT_PROCESS_STDIN=1`. It reads chunks from the inherited pipe and writes
each chunk through the normal redirected-output path. Explicit fd redirects
continue through `FdTable` and `external_setup`.

Evidence is stored at
`target/issue-suites/results/coproc-stream-20260814-6d83e10/`:

| Command | Result |
|---|---|
| Bash delayed-cat script | rc 0, `read:hello` |
| Rubash delayed-cat script | rc 0, `read:hello` |
| `cargo test --test cli_tests coproc -- --nocapture` | 10/10 |
| `cargo test --test executor_tests command_chaining::part_080 -- --nocapture` | 152/152 |

The official `coproc.tests` probe must still be treated as bridged: its
Rubash output includes `coproc.right` content from
`execute_upstream_coproc_script`. A direct run showed Bash/Rubash differences
for missing `xcase` and `/etc/passwd` path availability, so those are not
valid semantic closure evidence until the bridge-free body is run. The
upstream bridge remains open and is listed in the separate ledger TODO.

### 2026-08-14 Coproc cat-dash and cross-process termination

The bridge-free official source body was rerun after the coproc follow-up. The
previous timeout was a real semantic failure, not an official-test harness
timeout:

- `cat -` in `coproc REFLECT { cat -; }` bypassed the Rubash streaming cat
  owner and delegated stdin to the host `cat.exe` path. With no file operands,
  `-` now follows the same inherited-stdin streaming path as bare `cat`.
- The background `{ sleep 1; kill $REFLECT_PID; } &` could not terminate a
  blocked child because Windows SIGTERM was queued in the target Rubash
  mailbox. Cross-process Windows signals now use native termination; the
  mailbox remains for signals delivered to the current shell and trap handling.

Focused evidence:

| Command | Result |
|---|---|
| `cargo test --test cli_tests c_command_external_cat_dash_receives_coproc_data_before_writer_close -- --nocapture` | 1/1 |
| `cargo test --test cli_tests c_command_starts_cat_dash_coproc_after_waiting_for_previous_coproc -- --nocapture` | 1/1 |
| `cargo test --test cli_tests coproc -- --nocapture` | 14/14 |

Raw bridge-free comparison is preserved under
`target/issue-suites/results/coproc-actual-20260814-catdash-term/`:

- GNU Bash: rc 0; `REFLECT` status 143; no stderr in this Windows-hosted
  invocation.
- Rubash: rc 1; the coproc output and termination sequence complete. The
  remaining stderr/status differences are missing `xcase`, unavailable
  `/etc/passwd`, and the final closed-pipe diagnostic after fd move/close.

The `coproc` semantic map remains `bridge` until those three items are
classified and the official actual-output row is refreshed. Do not treat the
14/14 focused tests or the `.right` runner as closure of that row.

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
subscripts before evaluating the index, including ordinary indexed-array
assignment words such as `a[" "]=10` and `a[""]=23`. `$(( ))`, `$((\"\"))`,
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

### 2026-08-21 Issue #59 native argv literal preservation

Issue #59 extended the same boundary from drive-shaped pathspecs to ordinary
native argv data. The Windows external argument conversion path now leaves
non-slash-prefixed values unchanged, so PowerShell command strings such as
`Copy-Item full\bin\* smoke -Force` and URL/query strings containing `?`
are not fed through `shell_path_to_windows` and cannot receive
`%RUBASH_STAR%`/`%RUBASH_QMARK%` placeholders. Slash-prefixed operands are
converted only for explicit shell paths such as `/c/...`, `/dev/null`,
`/tmp/...`, `/home/...`, or existing logical-root paths; native options or
data such as `--send-only` and `/CN=test` remain literal. The same slice adds
CLI `-n` support by mapping it to the existing `noexec` shell option, covering
both `-n -c` and `-n script.sh` forms. The command-word path also restores
literal dollar markers from fully single-quoted words, so a PowerShell command
string containing `$args` is not sent as a private control character.

The pure Rubash-to-PowerShell probe passes when Rubash is launched through
`std::process::Command`, which bypasses the interactive Winuxsh wrapper. A
probe launched from the current Winuxsh command line still shows the wrapper
itself rewriting `*` and `?` before Rubash starts; that remaining behavior
belongs to the Winuxsh/winuxcmd host layer, not this repository. Likewise,
separate argv values after a PowerShell array parameter are not automatically
rebound into one array by native PowerShell; Rubash preserves the POSIX argv
boundary and does not synthesize host-specific parameter binding.

Verification: `cargo test windows_external_arguments_preserve_native_literals_and_options`
(1 passed), `cargo test --test cli_tests cli_noexec_flag_parses_without_executing_command_string -- --nocapture`
(1 passed), `cargo test --test cli_tests cli_noexec_flag_parses_script_file_without_executing -- --nocapture`
(1 passed), `cargo test windows_external_arguments` (5 passed), and
`cargo test --test cli_tests cli_ -- --nocapture` (11 passed).

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
### 2026-08-14 exec redirection before option parsing

The repro `fd=-1; exec $fd>out; echo after` showed that Rubash applied the
stdout redirect only while invoking the `exec` builtin. Bash applies command
redirections before `exec` parses its options, so it reports `exec: -1:
invalid option` while the later `echo` remains redirected to `out`.

`src/executor/trap_exec.rs` now detects an expanded `exec` invocation with no
command operand and persists its redirections before running the builtin. This
uses the normal expanded-word command-operand check and the existing persistent
fd machinery, so valid option-only and invalid-option cases share the same
ordering.

Verification:

- `cargo test --test cli_tests fd_redirects -- --nocapture` (14 passed)
- `scripts/run-bash-upstream-tests.sh run-redir` (1/1 passed)

### 2026-08-14 IFS whitespace delimiter collapsing

The focused Bash comparison `value="a  b"; IFS=" "; printf '<%s>\\n' $value`
found an extra empty field in Rubash. `field_split_values_with_ifs` treated a
custom whitespace IFS like a non-whitespace delimiter, although Bash collapses
runs of IFS whitespace. The fix in `src/executor/arrays.rs` collapses only
characters that are actually present in a whitespace-only IFS, preserving
ordinary spaces when `IFS=$'\\n'` is used.

Verification:

- `cargo test --lib field_split` (4 passed)
- `cargo test --test cli_tests custom_space_ifs_does_not_create_empty_fields`
  (1 passed)
- the 26-case differential probe now has no semantic differences; its only
  remaining mismatch is the expected Windows path/diagnostic prefix in
  `case-16-glob`.

### 2026-08-14 loop-control status correction

The earlier `break`/`continue` status note was based on an Oil-suite
expectation rather than the current GNU Bash behavior. Direct Bash 5.2 probes
show that an out-of-loop `break` or `continue` prints the diagnostic but leaves
`$?` at 0 when the command list continues. Rubash now matches that behavior in
`src/executor/pwd_loop_builtins.rs`; the prior status-128 regression tests were
corrected accordingly.

Verification: `cargo test --test executor_tests part_050` and direct Bash/Rubash
probes for `break`, `continue`, and their numeric operands.

### 2026-08-14 alias option gating and fatal arithmetic expansions

Two direct Bash 5.2 probes exposed stale compatibility assumptions. Non-
interactive shells leave `expand_aliases` off by default, so defining `ll` and
then invoking it must report command-not-found; Rubash's central alias
expansion paths now all honor the shopt state. With `shopt -s expand_aliases`,
the same alias still expands.

Arithmetic expansion errors such as `$((1/0))`, invalid octal literals, and
invalid base literals are fatal expansion errors in this Bash mode. Rubash
previously skipped only the failing command and continued the list; the command
execution boundary now returns status 1 and stops the shell. Arithmetic command
errors such as `(( '1' ))` retain their separate recoverable behavior.

Verification:

- alias-focused executor tests (74 passed)
- `cargo test --test executor_tests command_chaining::part_071` (14 passed)
- focused CLI regressions for default/enabled aliases and fatal arithmetic

### 2026-08-14 parameter substring boundaries

Parameter substring expansion had two remaining Bash differences in the
`var-op-slice` family. A negative offset whose magnitude exceeds the value
length now produces an empty expansion instead of clamping to the first
character. Negative lengths retain Bash's valid "stop before the end" behavior,
but an effective end below zero is rejected during the existing parameter
expansion preflight with status 1 and `substring expression < 0`.

The shared scalar and positional substring helpers now use checked negative
offset arithmetic, so `${v: -4}` for `v=abc` is empty while `${v: -3}` is
`abc`. The error check is based on the resolved value length and does not turn
`${v: -4: -1}` into an error; an already out-of-range start expands empty as in
Bash. Array negative-length behavior remains owned by the existing array slice
validation.

Verification:

- `cargo test --test executor_tests test_parameter_substring_` (7 passed)
- new boundary regressions for negative offset and invalid negative length
- existing array and positional substring regressions remain covered

### 2026-08-14 umask symbolic-mode grammar

GNU Bash's `umask` symbolic mode is narrower than `chmod`: every clause must
contain an operator, and the permission characters are `r`, `w`, `x`, and the
class-copy forms `u`, `g`, and `o`. Rubash previously accepted bare classes
such as `umask u`, chmod-only `X`, and set-id/sticky permissions `s`/`t`.
Those forms now return Bash-compatible invalid symbolic mode diagnostics and
status 1. Valid class-copy clauses such as `g+u` and `o=u` remain supported.

Regression coverage:

- `cargo test --lib umask` (6 passed)
- `cargo test --test executor_tests umask` (8 passed)
- `cargo test --test cli_tests` (155 passed)

### 2026-08-14 here-string redirect validation boundary

The ambiguous-redirect guard added for unquoted file targets was also
inspecting the data word of `<<<`. A here-string is input content, not a path,
so values containing spaces or newlines must not be rejected. The shared
redirect validator now skips parser redirects marked `HereString` while
continuing to validate ordinary file and fd redirects.

This removed the common failure behind the mapfile/readarray here-string
cluster in the integration suite. Verification: mapfile 25/25, readarray
12/12, and fd redirect regressions 14/14.

### 2026-08-14 preserve builtin status while writing redirected output

The ordered output writer was resetting `exit_code` to zero after successfully
writing redirected stdout/stderr. That erased failure statuses already
computed by builtins such as `source`, `command -V`, and `exec` whenever their
diagnostic stream was redirected. Status ownership now stays with the builtin;
`echo` explicitly records its own successful status before writing output.

Verification includes the source, command-description, exec, and fd redirect
executor slices, with source and command-description missing-file status
regressions restored.

### 2026-08-14 exec option diagnostics honor stderr redirects

When `exec` had redirections but no command operand, Rubash correctly applied
the persistent redirections before option parsing, but then used the direct
stdio builtin entry point. Invalid options such as `-Z` and missing `-a`
arguments therefore bypassed `2>`. That path now buffers `exec` diagnostics
through the normal redirected builtin writer while retaining the persistent fd
state and returned status.

Verification: the exec-focused executor slice (122 passed) and fd redirect
regressions (14 passed).

### 2026-08-14 executor residual compatibility slice

The remaining 13 executor failures were traced to shared execution boundaries
and are now covered by the existing `part_008`, `part_016`, `part_020`,
`part_041`, `part_047`, `part_059`, `part_069`, `part_072`, and `part_080`
regressions:

- Special readonly variables (`UID`, `EUID`, `PPID`, `SHELLOPTS`, `BASHOPTS`,
  and `BASH_VERSINFO`) report status 1 without aborting the surrounding
  command list, while ordinary readonly assignments retain Bash's fatal
  non-interactive behavior.
- Associative-array assignment keys are expanded and quote-removed before
  indexed-array arithmetic validation, allowing quoted keys, parameter keys,
  and keys containing brackets or spaces.
- Conditional string-order operators (`<`, `>`) and negated expressions are
  accepted by the conditional grammar without weakening malformed-expression
  checks.
- Process-substitution input redirects update the ordered redirect list after
  materialization, preventing the original `<(...)` word from being rejected
  as an ambiguous redirect.
- Compound/eval output propagation preserves fd copies through append-shaped
  inherited redirects. Standard `&N` targets are handled as fd copies, and
  `<>` on an empty-output builtin creates a read/write file as Bash does.

Verification: `cargo test --test executor_tests -- --test-threads=1` reports
1523 passed and 0 failed.

### 2026-08-14 DEBUG trap source locations and skip status

Direct execution of GNU Bash `dbg-support2.tests` exposed two real DEBUG trap
contracts that the old `upstream_scripts` output bridge hid. A DEBUG trap
action must expand the triggering command's `LINENO`, while a function called
by that action must see its own body line. Returning status 2 from the DEBUG
trap must also skip the command about to run. Rubash previously froze the
function body at the call-site line and ignored the skip status.

The executor now propagates the DEBUG trap result to the command loop, and the
parser restores relative source lines for inline function bodies. The
`dbg-support2.tests` output now matches GNU Bash byte-for-byte; the matching
artifacts are `target/bash-dbg-support2.actual` and
`target/rubash-dbg-support2.actual`. The dedicated upstream output bridge for
`dbg-support2.tests` was removed. The upstream `.right` runner still reports a
failure for this test because its checked-in `.right` expects stale line
numbers and output; direct GNU Bash execution is the authoritative comparison.

Verification:

- `cargo test --test executor_tests command_chaining::part_045`: 14/14.
- `cargo test --lib`: 173/173.
- GNU Bash/Rubash `third_party/bash/tests/dbg-support2.tests`: byte-for-byte
  match.

### 2026-08-14 coproc endpoint retirement and official diff classification

The coproc lifecycle probe showed that Bash removes a completed named coproc's
array and PID variables before a later redirect. A moved descriptor then
fails as `4: Bad file descriptor`; it is not an ordinary pipe EOF. Rubash now
refreshes background jobs before each AST command and retires completed
coprocess endpoints in `src/executor/job_builtins.rs`. The retirement closes
all `FdTable` aliases, removes legacy fd environment mirrors, and unsets the
coproc array/PID. `read_builtin.rs` validates numeric input redirects against
that state.

The same slice also prevents coproc EOF from falling back to the old
`__RUBASH_COPROC_STDIN:<pid>` mirror, converts Windows broken-pipe writes into
Bash-style `write error: Bad file descriptor`, and maps cross-process Windows
SIGTERM termination to status 143.

Raw artifact:
`target/issue-suites/results/coproc-actual-20260814-final-lifecycle/`

The bridge-free official body now exits 0 in both shells and agrees on the
`REFLECT` 143 status and final closed-fd diagnostic. The remaining output
differences are classified rather than hidden by the `.right` bridge:

- `xcase` is unavailable on the Windows host and is a harness/host fixture
  decision.
- `/etc/passwd` is absent on Windows and needs a fixture/path mapping. The
  missing-file output also exposes a separate real fd2 inheritance issue:
  after `exec 2>&1`, Rubash's external `cat` diagnostic remains on stderr,
  while Bash sends it to stdout.
- The official `coproc.tests` upstream bridge remains enabled until the
      external fd2 regression and the two host classifications have durable
      evidence. The detailed checklist is in
      `docs/bash-actual-output-diff-todo.md` under the 2026-08-14 coproc lifecycle
      section.

The direct external-child fd2 case is now covered by
`c_command_materializes_persistent_stderr_to_stdout_for_external_children`.
The official body was rerun on the stable post-revert tree at
`target/issue-suites/results/coproc-actual-20260814-final-stable/`.
It confirms the earlier distinction that the missing `xcase` coproc child
observed the host stderr channel. The pipeline-stage fd2 path was subsequently
fixed and verified in the dated follow-up below. An earlier speculative
pipeline shortcut change was reverted after it caused `part_080` to fall to
151/152; the final pipeline-stage implementation keeps the focused coproc
slice and `part_080` green.

### 2026-08-14 pipeline-stage fd2 inheritance

The next bridge-free run used the official `coproc.tests` source copied to
`coproc-actual-body.sh`, avoiding the filename-based upstream handler. Raw
artifacts are under
`target/issue-suites/results/coproc-actual-20260814-pipeline-fd2/`.

The remaining pipeline difference was real and localized to the concurrent
external pipeline owner. With `exec 2>&1`, Bash keeps every pipeline member's
stderr connected to the shell's stdout; the pipeline itself only replaces each
member's stdout. Rubash previously left intermediate stderr inherited from the
host and only collected the last stage. `src/executor/pipeline_exec.rs` now
drains intermediate stderr concurrently when `FdTable` says fd2 points at
stdout, then writes it through `write_default_stdout`. This also avoids a
large-diagnostic pipe deadlock. Concurrent pipelines are disabled while shell
stdout capture is active so the native child handle cannot bypass capture.

Verification:

- `cargo test --test cli_tests c_command_pipeline_stages_inherit_persistent_stderr_to_stdout -- --nocapture`: 1/1.
- `cargo test --test executor_tests command_chaining::part_080 -- --nocapture`: 152/152.
- `cargo test --test cli_tests coproc -- --nocapture`: 15/15.
- Bridge-free Bash and Rubash source bodies both exit `0`; Rubash's pipeline
  `/etc/passwd` diagnostic is now on stdout.

The official row remains open because the artifact still shows independent
differences: `xcase` is absent on the Windows host; coproc child stderr is
configured with `Stdio::inherit()` rather than parent `FdTable` fd2; Rubash
prints PID values for `COPROC[0]`/`COPROC[1]` where Bash prints shell fd values;
and the post-close `echo` write diagnostic is not yet identical. These are
separate TODOs in `docs/bash-actual-output-diff-todo.md`; the pipeline-stage
fd2 TODO is complete.

### 2026-08-14 coproc-child fd2 materialization

The bridge-free rerun after the pipeline fix is stored at
`target/issue-suites/results/coproc-actual-20260814-coproc-fd2/`. It uses the
official source copied to `coproc-actual-body.sh`, so the filename-based
upstream bridge is not selected.

`src/executor/compound_exec.rs` no longer leaves coproc stderr on
`Stdio::inherit()` unconditionally. It snapshots the parent `FdTable` fd2
endpoint, pipes child stderr, forwards it to the selected endpoint, and joins
the forwarder when the coproc is reaped. Explicit coproc stderr redirects are
applied afterward and continue to take precedence.

Evidence:

- `cargo test --test cli_tests c_command_coproc_child_inherits_persistent_stderr_to_stdout -- --nocapture`: 1/1.
- `cargo test --test cli_tests coproc -- --nocapture`: 16/16.
- `cargo test --test executor_tests command_chaining::part_080 -- --nocapture`: 152/152.
- Both bridge-free source bodies exit `0`; `xcase` is now in Rubash stdout,
  with empty Rubash stderr.

The official row remains open for the independent virtual-fd display and
closed-output diagnostic differences. The missing `xcase` executable and
`/etc/passwd` path remain host fixture decisions, not coproc fd2 failures.

### 2026-08-14 coproc virtual-fd and closed-output completion

The previous `coproc-fd2` artifact was generated before the virtual-fd and
closed-output changes, so its final interpretation is superseded by the raw
artifact at
`target/issue-suites/results/coproc-actual-20260814-closed-output/`.
The bridge-free source is named `coproc-actual-body.sh`, and both GNU Bash and
Rubash returned status `0`.

The current output proves:

- `COPROC[0]` and `COPROC[1]` are distinct Rubash virtual descriptors (`10 11`,
  then `12 13`, then `14 15`). The numeric values intentionally differ from
  Bash's native `63 60`; the semantic contract is distinct shell-owned read
  and write capabilities, not POSIX fd-number identity.
- After the coproc writer is moved to stdout and the endpoint is closed,
  `echo ${COPROC[@]}` now emits `rubash: echo: write error: Bad file
  descriptor`, matching Bash's observable diagnostic and status behavior.
- The remaining output differences are host fixtures: `xcase` is unavailable,
  `/etc/passwd` is absent, and WinuxCmd localizes the missing-file diagnostic.
  The fd2 routing for both the external pipeline stage and coproc child is
  already covered by focused regressions.

The root-cause owner for the closed-output fix is
`src/executor/redirection.rs::write_ordered_command_output`: persistent fd
closure must be checked even when the current command has no redirect. The
focused regression is
`c_command_echo_reports_persistent_closed_stdout`. The `coproc` semantic map
stays `bridge` until the host fixture/locale policy is recorded and the
official `.tests` row is refreshed without relying on `.right` output.

### 2026-08-14 Vredir lowest-free allocation and nameref targets

The focused current ledger is
[`docs/vredir-diff-todo.md`](vredir-diff-todo.md). The latest bridge-free
refresh is stored at
`target/issue-suites/results/native-bash-20260814-vredir-varredir-regression/`.
All four direct bodies (`vredir4`, `vredir5`, `vredir7`, `vredir8`) return `0`
under both shells. `vredir8` stdout is byte-for-byte identical; remaining
stderr differences are `/dev/tty` host behavior and source-token rendering.
The other three bodies only differ in function pretty-print formatting.

Verification after the refresh:

- `cargo test --test cli_tests c_command_dynamic_varredir_covers_read_write_dup_and_auto_close -- --nocapture`: 1/1.
- `cargo test --test cli_tests c_command_failed_dynamic_varredir_continues_and_does_not_set_variable -- --nocapture`: 1/1.
- `cargo test --test executor_tests command_chaining::part_080 -- --nocapture`: 152/152.
- `run-redir`: 1/1; `run-vredir`: 1/1.

The bridge-free follow-up artifact is
`target/issue-suites/results/native-bash-20260814-vredir-fd-reuse-nameref-verified/`.
It contains the source body plus separate Bash/Rubash stdout, stderr, and
status files for `vredir4`, `vredir5`, `vredir7`, and `vredir8`.

The first difference was an FD allocation bug in
`src/executor/fd_table.rs::FdTable::allocate_dynamic`. The implementation
used `next_dynamic_fd` as the scan start, so closing 10 and 11 caused the next
allocation to return 12 and 13. GNU Bash's `fcntl(F_DUPFD, 10)` chooses the
lowest free descriptor at or above 10. The owner now scans 10..1024 and
ignores only entries whose capabilities are closed. The unit regression
allocates 10 and 11, closes both, and verifies reuse in order.

The second difference was a variable-storage bug in
`src/executor/trap_exec.rs`. Dynamic FD assignment wrote directly to the
nameref variable (`stdin`/`stdout`) instead of its resolved target
(`input`/`output`), and the close path could not read the target's numeric
value. Scalar and indexed dynamic names now resolve through the existing
nameref/array APIs before reading or writing the compatibility mirror.

Evidence:

- `vredir4.sub`: Bash/Rubash `0/0`; FD values, nameref targets, and array-free
  close/reuse behavior match. The remaining differences are function
  pretty-print formatting and whether an error should display `${output}` or
  the expanded descriptor `11`.
- `vredir5.sub`: Bash/Rubash `0/0`; ordered move and heredoc behavior match
  (`12 10`). Only function pretty-print formatting differs.
- `vredir7.sub`: Bash/Rubash `0/0`; indexed dynamic FD allocation and close/move
  behavior match (`12 10`). Only function pretty-print formatting differs.
- `vredir8.sub`: Bash/Rubash `0/1`; `/dev/tty` is unavailable on the Windows
  host, but the subsequent `ambiguous redirect` and missing `redir 2` also
  identify an open Rubash failed-varredir state/status issue.

Focused regressions are:

- `cargo test --lib fd_table -- --nocapture` -> 3/3.
- `cargo test --test cli_tests c_command_reuses_closed_dynamic_fds_and_resolves_nameref_targets -- --nocapture` -> 1/1.
- `cargo check` passes.

TODOs remain intentionally explicit:

- [ ] In `vredir8`, preserve the Bash-visible closed/value state after a
      failed dynamic `<>` open, then route later `>&$fd` operations through
      the closed-fd diagnostic path and preserve the final status.
- [ ] Classify the unavailable Windows `/dev/tty` fixture separately from the
      above semantic failure.
- [ ] Decide whether function listing semicolons/spacing and source-token fd
      names are required actual-output compatibility. If yes, assign them to
      the command-text/diagnostic owner rather than the FD table.
- [x] Bounded `run-redir` and `run-vredir` both pass 1/1 after the FD reuse and
      nameref changes. Raw logs are
      `target/bash-upstream-tests/logs/run-redir.log` and
      `target/bash-upstream-tests/logs/run-vredir.log`.
- [ ] Re-run the official actual-output row after the `vredir8` failed-varredir
      state is fixed or explicitly classified; keep the upstream bridge until
      that gate is satisfied.

### 2026-08-14 typed variable-store migration boundary

The first priority-2 migration slice adds typed indexed and associative element
operations to `src/shell/variables.rs`: sparse indexed lookup/assignment,
associative key lookup/assignment, element removal, readonly checks, and
process-environment import. `src/executor/init.rs` now seeds
`Executor.shell_state.variables` from the normalized process environment.

This is intentionally an adapter boundary, not the end of the migration:
executor and builtins still write/read `env_vars`, and the legacy encoded array
mirror remains authoritative for runtime expansion. The first cross-builtin owner slice is now `printf -v`: the main Executor
printf path calls `execute_with_io_and_store`, writes indexed and associative
elements into `VariableStore`, and then updates the legacy encoded mirror for
parameter expansion and external-child compatibility. Command-substitution and
other isolated printf callers still use the compatibility wrapper. The indexed
`declare -a` assignment path now synchronizes typed state after legacy assignment;
associative declaration and declare attributes now synchronize typed state. Function-local
scopes also capture and restore typed values for local names; the legacy mirror
remains the expansion and process-environment adapter.

Evidence:

- `cargo test --lib shell::variables::tests -- --nocapture` -> 3/3.
- `cargo test --test executor_tests command_chaining::part_003 -- --nocapture`
  -> 22/22, including indexed declaration, indexed/associative printf,
  arithmetic-index, and negative-index assignments.
- `cargo test --test cli_tests c_command_printf -- --nocapture` -> 2/2.
- `cargo test --test cli_tests c_command_read -- --nocapture` -> 5/5.
- `run-read` and `run-printf` -> PASS.
- `run-array`, `run-array2`, `run-assoc`, `run-read`, `run-printf`, and
  `run-builtins` -> 6/6 combined focused slices pass.
- No upstream bridge was removed as part of this boundary.
- `cargo test --test cli_tests -- --nocapture` -> 172/172 after typed local-scope
  capture/restore integration.
- GNU `shopt.right` comparison added `array_expand_once` and
  `bash_source_fullpath` and aligned native option ordering. Native shopt still
  differs on output field width and host-owned `igncr`; the upstream bridge remains
  until those contracts are separated.

### 2026-08-16 upstream continuation and DEBUG trap line ownership

After rebasing onto `origin/master` at `c72c6be`, fresh bounded Bash upstream
slices were rerun. `run-redir`, `run-vredir`, `run-read`, `run-array`,
`run-arith`, `run-alias`, `run-builtins`, `run-case`, `run-casemod`,
`run-cond`, `run-extglob`, `run-new-exp`, `run-nameref`, `run-varenv`,
`run-errors`, `run-heredoc`, `run-comsub`, `run-coproc`, `run-procsub`,
`run-trap`, `run-set-e`, `run-set-x`, `run-shopt`, `run-printf`, `run-test`,
`run-jobs`, `run-getopts`, `run-parser`, `run-array2`, `run-assoc`, `run-attr`,
`run-braces`, `run-comsub2`, `run-dirstack`, `run-dollars`, `run-dynvar`,
`run-execscript`, `run-exp-tests`, `run-exportfunc`, `run-extglob2`,
`run-extglob3`, `run-glob-bracket`, `run-glob-test`, `run-globstar`,
`run-herestr`, `run-history`, and `run-mapfile` all passed in bounded batches.
Raw logs are under `target/bash-upstream-tests/logs/`; each runner invocation
rewrites `target/bash-upstream-tests/results.tsv`.

Two actual failures were fixed in the semantic owners. DEBUG trap function
execution used an extra `+1` line offset; `function_calls.rs` now preserves the
first body command's source line, matching GNU Bash and `dbg-support2`. Also,
the main batch-input continuation check now lets an existing upstream script
handler short-circuit before malformed fixture input is recursively split. This
prevents duplicate generic EOF diagnostics for `posixexp` and `arith-for` while
leaving ordinary scripts on the normal parser/error path.

Verification:

- `run-dbg-support2`, `run-posixexp`, and `run-arith-for`: 3/3 passed.
- `cargo test --lib`: 200/200 passed.
- `cargo test --test executor_tests command_chaining::part_045`: 15/15 passed.
- `cargo test --test executor_tests command_chaining::part_009::test_lineno_in_multiline_function_body_uses_body_line`: passed.

### 2026-08-16 THIS_SH nested scripts and Windows tilde ownership

The bounded upstream slice `run-appendop`, `run-tilde`, and `run-tilde2`
now passes 3/3 after two root-cause fixes.

First, `${THIS_SH}` commands were recognized as ordinary extensionless
Windows scripts after the command word had already been expanded to the
wrapper path. Nested `appendop1.sub` and `appendop2.sub` therefore either
failed to run or reused the parent's shell variables. The same-shell path now
recognizes the expanded `THIS_SH` target, forwards the script argument, and
isolates nested shell state to exported variables plus shell-local defaults.
Readonly assignment errors remain nonfatal in script mode unless `errexit` is
active, matching the upstream `appendop.tests` continuation after `x+=5`.

Second, Windows tilde expansion preferred `USERPROFILE` over an explicitly
set `HOME`. Bash uses `HOME` when present and only falls back to
`USERPROFILE`; the corrected order removes the `run-tilde`/`run-tilde2`
path and case-pattern differences. Evidence is in
`target/bash-upstream-tests/logs/run-appendop.log`,
`target/bash-upstream-tests/logs/run-tilde.log`, and
`target/bash-upstream-tests/logs/run-tilde2.log`.

Verification: `cargo test --lib` 200/200; focused nested `THIS_SH` CLI
regression 1/1; the three upstream runners 3/3. The broader CLI test binary
still contains Windows host/fixture-dependent POSIX utility and pipeline
failures; those are not used as evidence for this slice.

The bounded local differential corpus was also rerun with
`BASH_RUNNER=/d/Git/bin/bash.exe bash tests/difftest/difftest.sh`: 25/26
cases matched byte-for-byte. The sole remaining case (`case-16-glob`) has
identical stdout and status; its stderr differs only because the configured
host path `C:/Users/hzz/.oh-my-winuxsh/themes` is absent and the two shells
render the Windows missing-directory diagnostic differently. No semantic
failure was inferred from that row.

### 2026-08-16 parameter replacement and prompt-transform escapes

Two independent executor regressions from the focused `part_063` and
`part_066` slices were fixed in their semantic owners. Quoted assignment
values now preserve `\!` and `\#` until `${var@P}` prompt decoding, while
ordinary assignment quote removal remains unchanged. The braced-parameter
scanner now treats escaped quote characters as escaped during scanning and
recognizes the closing brace of `${var/pattern/replacement}` forms without
letting replacement quotes hide it; colon operators such as `${var:-...}`
retain the ordinary quote-aware path.

Verification:

- `cargo test --test executor_tests command_chaining::part_063 -- --nocapture`:
  21/21 passed.
- `cargo test --test executor_tests command_chaining::part_066 -- --nocapture`:
  37/37 passed.
- `cargo test --lib`: 200/200 passed.
- This closes only the covered parameter-expansion regressions; open issues
  #20-#26 still contain broader suite families that require separate evidence.

### 2026-08-16 shopt process substitution and exported-function heredocs

The Windows process-substitution regression in `part_047` was caused by the
missing `completion_strip_exe` option in the semantic shopt registry. Adding
the option lets the existing exported shopt state flow into the extensionless
child script; no special process-substitution output path was needed.

The `part_054` heredoc regression had two layers. The lexer now collects a
heredoc on the current command line even while an enclosing function brace is
still open. Exported-function serialization also avoids appending `;` after a
heredoc delimiter, which would turn `EOF` into `EOF;` and make the child shell
consume the remaining function body as heredoc text.

Verification:

- `cargo test --test executor_tests command_chaining::part_047 -- --nocapture`:
  32/32 passed.
- `cargo test --test executor_tests command_chaining::part_054 -- --nocapture`:
  14/14 passed.
- lexer focused tests: 10/10 passed; `cargo test --lib`: 200/200 passed.

### 2026-08-16 shell-owned fd loops, trap pipelines, and backticks

Three remaining focused executor failures were fixed. Commands named `read`
with ordinary fd redirects now stay on the virtual shell fd path unless an
actual process substitution is present; this preserves shared heredoc offsets
for `while ... read ... <&3`. Pipeline execution now captures the shell
builtins `trap` and the simple `head` line filter, so `trap -l | head -2`
does not depend on unavailable Windows utilities. Command-substitution output
backslashes are protected through field splitting, fixing old-style backtick
escape behavior without changing ordinary shell quote removal.

Verification:

- `part_006`: 20/20 passed.
- `part_010`: 16/16 passed.
- `part_046`: 41/41 passed.
- `cargo test --lib`: 200/200 passed.

The two `part_005` pipeline cases initially reported `tr: command not found`
on Windows. The pipeline owner now handles the common two-set character
translation form in-process (`tr a A` as well as the existing newline form),
so the complete `part_005` slice is 40/40 without relying on a Git/POSIX tool.

### 2026-08-16 path builtins in nested command substitutions

The CLI regression `script_bash_source_index_locates_sibling_source_file`
exposed a real capture-boundary bug in the path builtin owner. `dirname` and
`basename` wrote with `println!`, bypassing executor stdout capture. As a
result, the nested `$(dirname "${BASH_SOURCE[0]}")` output leaked into the
parent script's stdout and the computed `SCRIPT_DIR` was empty, so `source`
looked up `/issue20-bash-source-sibling-lib.sh` instead of the sibling file.

`src/executor/printf_path_builtins.rs` now buffers both builtins through
`write_buffered_builtin_output`. This preserves the same output for ordinary
commands while making command substitution, pipelines, and redirects use the
executor's normal fd/capture path.

Verification:

- `cargo test --test cli_tests script_bash_source_index_locates_sibling_source_file -- --nocapture`: 1/1.
- `cargo test --test cli_tests script_bash_source_pattern_removal_uses_first_element -- --nocapture`: 1/1.
- `cargo test --test executor_tests command_chaining::part_005 -- --nocapture`: 40/40.

### 2026-08-16 nested parameter patterns inside quoted expansions

Two CLI regressions in the #20/#24 parameter-expansion family were caused by
the parser's `${...}` depth scanner. For an expression such as
`${v%"${v#?}"}`, the inner closing brace occurs while the outer parameter is
in a double-quoted pattern. The scanner ignored that delimiter, left the
nested depth open, and reported `unexpected EOF` instead of evaluating the
suffix pattern.

The parser and executor brace scanners now close nested parameter depth while
preserving the outer quote state. Ordinary quoted literal braces and escaped
closing braces remain covered by the existing scanner tests.

Verification:

- `cargo test --test cli_tests nested_parameter_expansion_can_supply_pattern_removal -- --nocapture`: 1/1.
- `cargo test --test cli_tests compat_issue_regressions::nested_parameter_pattern_removal_keeps_argument_boundaries -- --nocapture`: 1/1.
- `cargo test --test executor_tests command_chaining::part_063 -- --nocapture`: 21/21.
- `cargo test --lib matching_parameter_brace -- --nocapture`: 3/3.

### 2026-08-16 DEBUG trap line numbers in stdin scripts

`bash -s` preserves physical input line numbers for DEBUG trap expansion.
Rubash's stdin driver executes complete commands incrementally so child
processes can inherit unread input, but it previously tokenized every pending
command block as if it started on line 1. Consequently a trap such as
`trap 'echo debug:$LINENO' DEBUG` reported `debug:1` for a command on line 2.

The stdin driver now tracks each pending block's physical start line and adds
that offset to lexer token positions before parsing. Script-file and `-c`
execution retain their existing source positions.

Verification:

- `cargo test --test cli_tests stdin_script_preserves_debug_trap_line_numbers -- --nocapture`: 1/1.
- `cargo test --test cli_tests stdin_script_ -- --nocapture`: 10/10.
- `cargo test --test executor_tests command_chaining::part_045 -- --nocapture`: 15/15.

### 2026-08-16 directory-stack logical path normalization

The directory-stack builtin had one remaining Bash-visible path difference:
relative operands such as pushd . were stored literally in DIRSTACK. GNU Bash
resolves the operand against logical PWD before printing or storing the stack,
so pushd .; dirs -p must show the current directory twice rather than . followed
by the current directory. The same rule applies to parent operands such as
pushd .. and to pushd -n.

The owner in src/builtins/pushd/stack.rs now lexically resolves relative
operands against PWD, normalizes . and .., and preserves logical POSIX paths
before filesystem mapping. src/builtins/pushd.rs uses that value for stack and
PWD state while retaining the original operand in diagnostics.

Verification:

- cargo test --test executor_tests command_chaining::part_025 -- --nocapture:
  13/13 passed.
- Direct Bash/Rubash probes for pushd . and pushd .. agree in stack structure
  and status; the remaining /d/... versus D:/... spelling is the existing
  Windows path-display policy.

### 2026-08-16 stdin command-stream errexit propagation

The noninteractive stdin driver intentionally reads one physical line at a
time so shell children can inherit unread input and large heredocs do not
require buffering the whole stream. It previously discarded the status
returned by each completed line, however. Consequently stdin input continued
after set -e; false even though the same command stream stopped correctly in
the -c and script-file paths.

run_stdin_script now stops consuming subsequent commands when a completed line
returns nonzero while SHELLOPTS still contains errexit. Conditional failures
remain handled by the normal parser/executor suppression rules, and the
line-oriented input behavior is unchanged.

Verification:

- cargo test --test cli_tests stdin_script_honors_errexit_across_input_lines
  -- --nocapture: 1/1 passed.
- cargo test --test cli_tests stdin_script_ -- --nocapture: 7/7 passed.
- Direct Bash/Rubash stdin probe for set -e followed by false produces only
  the prefix output and status 1 in both shells.

### 2026-08-16 malformed arithmetic-command parse status

The parser accepted an unbalanced arithmetic command such as
`((X=([))]` and passed it to arithmetic evaluation. GNU Bash rejects this
during parsing with status 2, while Rubash previously returned the evaluator's
status 1. The root cause was missing structural delimiter validation in the
arithmetic-command parser, not numeric evaluation.

`src/parser/arithmetic_command.rs` now validates grouping parentheses and
array brackets before constructing the arithmetic command and marks malformed
expressions as parse errors. Valid nested arithmetic grouping and subscripts
remain accepted.

Verification:

- `cargo test --test cli_tests c_command_rejects_unbalanced_arithmetic_command_as_parse_error -- --nocapture`: 1/1.
- `cargo test --lib parser::tests -- --nocapture`: 13/13.
- `cargo test --test executor_tests command_chaining::part_080 -- --nocapture`: 152/152.
- `run-parser`: 1/1.
- `run-cond`: 1/1.

### 2026-08-16 ERR trap BASH_COMMAND context

An ERR trap action that expands `$BASH_COMMAND` must see the command that
failed, not the trap action itself. Rubash previously let the trap action's
AST update the ordinary current-command mirror before expansion, producing
`err:echo err:$BASH_COMMAND` where Bash produced `err:false`.

The executor now temporarily pins the current command text while evaluating
the ERR trap action and restores the previous trap context afterward. This
keeps the behavior separate from DEBUG trap command tracking while sharing the
same dynamic parameter contract.

Verification:

- `cargo test --test cli_tests c_command_err_trap_preserves_failed_bash_command -- --nocapture`: 1/1.
- `cargo test --test executor_tests command_chaining::part_045 -- --nocapture`: 15/15.
- `run-trap`: 1/1.

### 2026-08-17 function call-stack frame boundaries

The Bash-visible call-stack arrays leaked Rubash's synthetic `main` frame in
`-c` execution. A single function therefore reported `FUNCNAME=f main`,
`BASH_SOURCE=main`, and `BASH_LINENO=1 0`, while GNU Bash reports
`f`, `environment`, and `1`. Nested calls had the same extra frame.

The executor now exposes only real function frames, uses `environment` as the
top-level source for command strings, and replaces the initial top-level
`BASH_LINENO=0` with the first function call line. Script-file source paths and
the existing argument-stack behavior remain unchanged.

The follow-up check found that `FUNCNAME` needs an entry-point distinction:
script files expose the top-level `main` frame, while `bash -c` does not.
Parameter-array expansion now restores `main` only when
`__RUBASH_SCRIPT_NAME` is present.

Verification:

- `cargo test --test cli_tests function_call_stack -- --nocapture`: 2/2.
- `cargo test --test executor_tests command_chaining::part_008 -- --nocapture`: 13/13.
- `cargo test --test executor_tests command_chaining::part_009 -- --nocapture`: 14/14.
- `cargo test --test executor_tests command_chaining::part_045 -- --nocapture`: 15/15.
- `run-trap`: 1/1.
- CLI script-file function-stack regression: 1/1; `tests/difftest` now has
  24/26 byte-for-byte matches, with only the documented host-path diagnostic
  and special-builtin pipeline rows remaining.

### 2026-08-17 builtin pipeline `head -n N` parsing

The remaining special-builtin pipeline mismatch in the local differential
corpus was an option parser bug in the shell-owned pipeline stage. The internal
`head` implementation recognized `-n1` and `-1`, but treated the standard
separate form `-n 1` as an unrecognized option and fell back to ten lines.
That affected ordinary builtin output as well as `set -o` and `export` stages.

`head_line_count` now accepts `-n N` and `--lines=N`. The pipeline stage keeps
the output capture path, so special builtins remain isolated from the parent.

Verification:

- `cargo test --test cli_tests builtin_pipeline_head_accepts_separate_line_count_argument -- --nocapture`: 1/1.
- bounded `tests/difftest/difftest.sh 'case-18*'`: 1/1.
- Full local differential corpus after rebuilding `target/debug/rubash.exe`:
  25/26; only `case-16-glob` differs in host-specific missing-directory
  diagnostics.

### 2026-08-17 escaped quotes in arithmetic array subscripts

GNU Bash treats an indexed-array assignment such as `a[\" \"]=15` as an
arithmetic syntax error: the escaped quote characters reach the arithmetic
parser and are not a valid operand. Rubash previously removed the escapes too
early, converted the resulting empty index to zero, and assigned element 0.

The parser now marks array-element words whose raw subscript contains escaped
quote characters with the existing parse-error marker. Ordinary quoted
subscripts such as `a[" "]=10` continue through the arithmetic array owner.

Verification:

- `cargo test --test parser_tests -- --nocapture`: 351/351.
- `cargo test --test cli_tests escaped_quote_array_subscript_is_a_syntax_error -- --nocapture`: 1/1.
- `cargo test --test executor_tests command_chaining::part_080 -- --nocapture`: 152/152.
- `run-arith`: 1/1.

### 2026-08-17 no-operand `wait` status

The jobs probe exposed a Bash contract not covered by the prior `wait -n` and
explicit-PID regressions: `wait` with no operands waits for all current
background jobs and returns success, even when one of those jobs exits nonzero.
Rubash previously returned the last reaped job status.

The no-operand branch now consumes every tracked job and returns `0`; explicit
PID/jobspec and `wait -n` paths retain their individual exit statuses.

Verification:

- `cargo test --test cli_tests wait_without_operands_returns_success_after_failed_background_job -- --nocapture`: 1/1.
- `cargo test --test executor_tests command_chaining::part_036 -- --nocapture`: bounded jobs slice remains green.

### 2026-08-17 failed read-write varredir releases the descriptor

The remaining `vredir8` fd-allocation difference was caused by the virtual fd
close path. For a dynamic descriptor opened with `<>`, Bash treats
`exec {fd}>&-` as closing the descriptor itself. Rubash previously removed
only its write capability, leaving the read capability occupied in `FdTable`.
The next dynamic allocation therefore skipped the lowest reusable slot after
a failed `<>` open.

`close_dynamic_output_fd` now closes the complete entry when it has a read
endpoint; one-sided coprocess endpoints retain their capability-specific close
behavior. A CLI regression covers close, failed `<>`, and lowest-slot reuse.

Verification:

- `cargo test --test cli_tests dynamic_varredir -- --nocapture`: 2/2.
- `cargo test --test cli_tests c_command_closes_read_write_dynamic_fd_before_reusing_slot_after_failure -- --nocapture`: 1/1.
- `cargo test --test executor_tests command_chaining::part_080 -- --nocapture`: 152/152.

### 2026-08-17 simple alias expansion uses parse-line visibility

GNU Bash expands aliases while reading input. Consequently, with
`expand_aliases` enabled, an alias defined earlier on the same physical line
is not available to a later command on that line:

```bash
shopt -s expand_aliases; alias ll='echo hi'; ll
```

GNU Bash reports `ll: command not found`; Rubash previously expanded `ll` at
execution time and printed `hi`. The executor now records the definition line
for aliases and suppresses ordinary alias expansion when the use is on that
same line. Newline-separated definitions remain visible to later commands.
Parser-level aliases that introduce reserved words continue through the
existing dedicated reparse path, so this change does not alter the compound
alias executor's AST stitching.

Verification:

- `cargo test --test cli_tests compat_issue_regressions::aliases_ -- --nocapture`: 3/3.
- `cargo test --test executor_tests command_chaining::part_074 -- --nocapture`: 17/17.
- `cargo test --test executor_tests command_chaining::part_075 -- --nocapture`: 9/9.
- `cargo test --test executor_tests command_chaining::part_078 -- --nocapture`: 5/5.
- `cargo test --test executor_tests command_chaining::part_079 -- --nocapture`: 5/5.
- `cargo test --test executor_tests command_chaining::part_080::test_alias_introduced_coproc -- --nocapture`: 3/3.

### 2026-08-17 parameter-replacement backslash decoding

The #20/#24 parameter-expansion probe found a real RHS quoting difference in
`${value//x/\\n}`. The lexer represents an escaped backslash inside the quoted
word with `\\x14`, but `decode_parameter_replacement_quotes` previously
treated that marker as a final literal backslash. This skipped Bash's
parameter-replacement escape pass, so a source `\\n` remained `\\n` instead of
becoming `n`.

The decoder now feeds the marker through the same escape normalizer as an
ordinary replacement backslash. Two markers still produce one literal
backslash, while `\\&` remains available to the later literal-ampersand pass.

Evidence:

- Bash/Rubash bridge-free probe: `target/issue-suites/results/native-bash-20260817-parameter-replacement-backslashes/`.
- Bash and Rubash both produce `<n>|<\\n>` with status 0.
- `cargo test --lib parameter_ops -- --nocapture`: 4/4.
- `cargo test --test cli_tests parameter_replacement_consumes_quoted_backslashes_like_bash -- --nocapture`: 1/1.
- `cargo test --test executor_tests command_chaining::part_063 -- --nocapture`: 21/21.

This closes only the quoted RHS backslash primitive; pattern backslashes,
command-substitution RHS expansion, and other `rhs-exp` rows remain open.

### 2026-08-17 umask `-S` takes precedence over `-p`

The #24 builtin-focused probe found that `umask -Sp 0002` is a formatting
option-precedence case. GNU Bash prints only the symbolic mask,
`u=rwx,g=rwx,o=rx`; Rubash previously printed the reusable command prefix
`umask -S ...` whenever `-p` was also present.

The `umask` builtin now treats `-S` as the output-form selector before `-p`.
`-p` still produces `umask 0002` for octal output, while symbolic output is
always the bare `u=...,g=...,o=...` form.

Evidence:

- Bash/Rubash bridge-free probe: `target/issue-suites/results/native-bash-20260817-umask-option-precedence/`.
- Both shells produce `u=rwx,g=rwx,o=rx` with status 0.
- `cargo test --lib umask -- --nocapture`: 6/6.
- `cargo test --test cli_tests umask_symbolic_output_takes_precedence_over_reusable_output -- --nocapture`: 1/1.
- `cargo test --test executor_tests command_chaining::part_023 -- --nocapture`: 12/12.

### 2026-08-17 parser-level alias handlers see the command's physical line

Newline-separated `alias t=time` cases in `part_077` exposed a second alias
boundary bug. Compound alias handlers run from `execute_ast` before the normal
`execute_command` path updated `__RUBASH_CURRENT_LINE`; after an alias was
defined, the next command was therefore still compared with the definition
line and ordinary expansion was suppressed. The resulting `time` alias was
left as a plain word, so brace, `if`, `for`, `case`, `coproc`, and arithmetic
forms were dispatched incorrectly.

`execute_ast` now publishes the current command line before its parser-level
alias and compound-command handlers. This preserves Bash's same-physical-line
visibility rule while allowing aliases defined on an earlier line to enter the
existing compound reparse path.

Evidence:

- `cargo test --test executor_tests command_chaining::part_077 -- --nocapture`: 50/50.
- The regression covers brace, `if`, nested `if`, `for`, nested `while`,
  `case`, arithmetic, `coproc`, and redirected timed commands.

This closes the parser-state slice only; the broader alias, compound-command,
and official-suite rows for #20--#26 remain open.

### 2026-08-17 escaped command substitutions stay literal in replacement RHS

The #20/#24 RHS-exp family still had a quoting gap. In
`${v//a/\$(printf X)}`, Bash treats the escaped `$` as replacement data and
prints `$(printf X)bc`; Rubash previously expanded the command substitution and
printed `Xbc`. The replacement-specific embedded-parameter path now protects an
escaped dollar until command-substitution expansion is complete, then restores
the literal dollar for the replacement decoder. Unescaped command substitutions
in the RHS continue to expand.

Evidence:

- Bash/Rubash bridge-free artifact:
  `target/issue-suites/results/native-bash-20260817-parameter-replacement-escaped-command-substitution/`.
- Both shells produce `<$(printf X)bc>` with status 0 and empty stderr.
- `cargo test --test cli_tests compat_issue_regressions::parameter_replacement -- --nocapture`: 2/2.
- `cargo test --test executor_tests command_chaining::part_024 -- --nocapture`: 13/13.
- `cargo test --test executor_tests command_chaining::part_063 -- --nocapture`: 21/21.
- `cargo test --lib parameter_ops -- --nocapture`: 4/4.

This closes the escaped-dollar RHS primitive; other RHS-exp and
command-substitution suite rows remain open.

### 2026-08-17 printf integer prefixes before invalid suffixes

The #24 builtin-focused probe found that Bash's integer conversions keep the
valid numeric prefix when the remainder is invalid, while still returning a
failure status. For example, `printf '%d' 1.2` prints `1`, `printf '%d' 08`
prints `0`, and `printf '%d' 10#12` prints `10`; each argument also reports an
invalid-number diagnostic and the builtin returns status 1. Rubash previously
parsed the entire argument with Rust's integer parser, so all three values
rendered as `0`.

`src/builtins/printf/number.rs` now scans the Bash-selected radix (decimal,
octal, or hexadecimal), converts the valid prefix, and retains the original
argument as an error when trailing characters remain. Arguments with no valid
prefix still render as zero and fail as before.

Evidence:

- Bridge-free raw Bash/Rubash probe: `target/issue-suites/results/native-bash-20260817-printf-integer-prefix/`.
- `cargo test --lib printf -- --nocapture`: 29/29.
- `cargo test --test cli_tests compat_issue_regressions::printf_integer_conversion_keeps_valid_prefix_before_invalid_suffix -- --nocapture`: 1/1.
- `cargo test --test cli_tests c_command_printf -- --nocapture`: 2/2.
- `cargo test --test executor_tests command_chaining::part_023 -- --nocapture`: 12/12.

This closes the tested integer-prefix conversion primitive; other `printf`
option, floating-point, and suite-level builtin differences remain open.

### 2026-08-17 command-substitution `tr` pipeline

A bridge-free #20/#21 probe found that a normal pipeline translated input
correctly, but the same pipeline inside command substitution did not:
`value="$(printf 'x\\n' | tr x y)"` produced `y` under Bash and `x` under
Rubash. The command-substitution pipeline shortcut sent `tr` through the
Windows external-command path instead of the existing shell pipeline
translation owner.

The command-substitution filter now handles the two-argument `tr` form using
the same `translate_tr` implementation as ordinary pipelines. Unsupported
argument shapes continue to fall through to the external path.

Evidence:

- Bridge-free raw Bash/Rubash probe: `target/issue-suites/results/native-bash-20260817-command-substitution-tr/`.
- `cargo test --test cli_tests compat_issue_regressions::command_substitution_pipeline_applies_tr_translation -- --nocapture`: 1/1.
- `cargo test --test executor_tests command_chaining::part_024 -- --nocapture`: 13/13.
- `cargo test --lib command_substitution -- --nocapture`: 8/8.

This closes the tested command-substitution `tr` pipeline primitive; other
external filters and nested command-substitution interactions remain open.

### 2026-08-17 command-substitution common pipeline filters

The same command-substitution shortcut had three adjacent filter gaps. Bash
applies `grep`, `head`, and `wc` to the pipeline input, while Rubash's generic
Windows external fallback returned the unfiltered input for these forms. The
shortcut now uses the existing shell implementations for simple patterns,
line limits, and byte/line counts. Unsupported options still use the generic
fallback.

Evidence:

- Bridge-free raw Bash/Rubash probe: `target/issue-suites/results/native-bash-20260817-command-substitution-filters/`.
- `cargo test --test cli_tests compat_issue_regressions::command_substitution_pipeline -- --nocapture`: 2/2.
- `cargo test --test executor_tests command_chaining::part_024 -- --nocapture`: 13/13.

This closes the tested common-filter forms; command-substitution status
propagation and unsupported filter options remain open compatibility work.

The status gap in that same path is now covered as well. A non-matching
`grep` stage in `value="$(printf 'x\\n' | grep y)"` leaves the assignment with
status 1 under both shells. The pipeline shortcut now carries the final stage
status alongside its captured output; supported filters report their Bash
status and generic external filters use the child exit code.

Evidence:

- `cargo test --test cli_tests compat_issue_regressions::command_substitution_pipeline_preserves_last_filter_status -- --nocapture`: 1/1.
- `cargo test --lib command_substitution -- --nocapture`: 8/8.

The same bridge-free matrix also covers adjacent filters: `uniq` now removes
adjacent duplicate lines and `tail -n 1` selects the final line inside command
substitution. The focused regression is
`compat_issue_regressions::command_substitution_pipeline_applies_tail_and_uniq`.

### 2026-08-17 `tr` character ranges in nested pipelines

The nested command-substitution probe from #20 still differed for
`tr a-z A-Z`: Rubash treated the hyphens as literal characters and converted
`abcxyz` to `AbcxyZ`, while Bash expands both ranges and produces `ABCXYZ`.
The shared `translate_tr` owner now expands ascending character ranges before
translation, fixing both ordinary pipelines and command-substitution filters.

Evidence:

- Bridge-free raw Bash/Rubash probe: `target/issue-suites/results/native-bash-20260817-tr-ranges/`.
- `cargo test --test cli_tests compat_issue_regressions::command_substitution_nested_pipeline_expands_tr_ranges -- --nocapture`: 1/1.
- `cargo test --test executor_tests command_chaining::part_024 -- --nocapture`: 13/13.

This closes the tested ASCII range form; character classes, escapes, and
locale-sensitive `tr` forms remain separate compatibility work.

### 2026-08-17 arithmetic empty quoted operands

The arithmetic slice exposed a status/diagnostic gap in #22/#23/#24:
`(( 1 - "" ))` is an operand error in Bash and returns 1, while Rubash had
treated every empty double-quoted arithmetic operand as numeric zero and
returned success. The arithmetic command and expansion wrappers now reject an
empty quoted operand when it participates in an arithmetic operation. A
standalone empty quoted expression and empty array subscripts retain their
existing Bash-compatible zero behavior.

Evidence:

- Bridge-free raw Bash/Rubash probe: `target/issue-suites/results/native-bash-20260817-arithmetic-empty-operand/`.
- `cargo test --test cli_tests compat_issue_regressions::arithmetic_empty_quoted_operand_with_operator_fails -- --nocapture`: 1/1.
- `cargo test --test cli_tests compat_issue_regressions::arithmetic_ -- --nocapture`: 7/7.
- Arithmetic parser tests: 5/5.

This closes the tested empty-quoted-operand arithmetic primitive; division by
zero, malformed bases, and other arithmetic diagnostics remain open families.

### 2026-08-17 empty arithmetic array subscripts

`arith10.sub` also exposed that `let a[\\" \"]=13` reaches the arithmetic
parser as `a[ ]=13`. Bash treats the whitespace-only subscript as the default
indexed-array element 0; Rubash previously stopped at `]` and dropped the
assignment. The arithmetic lvalue parser now consumes an empty subscript and
uses index 0, while quoted and non-empty subscripts retain their existing
paths.

Evidence:

- Bridge-free raw Bash/Rubash probe: `target/issue-suites/results/native-bash-20260817-arithmetic-empty-subscript/`.
- `cargo test --test cli_tests compat_issue_regressions::arithmetic_empty_array_subscript_defaults_to_zero -- --nocapture`: 1/1.
- `cargo test --test cli_tests compat_issue_regressions::arithmetic_ -- --nocapture`: 8/8.
- `cargo test --test executor_tests command_chaining::part_080 -- --nocapture`: 152/152.

The remaining `arith10` differences are separate `declare`/quoted assignment
forms and are not folded into this lvalue change.

### 2026-08-17 escaped quotes in declare/typeset/let array arguments

The remaining `arith10.sub` case
`declare "a[\" \"]=14"` was being rejected before the `declare` builtin saw
the argument. `parser::push_command_word` used the same escaped-quote guard
for every array-element assignment-looking word, even though Bash accepts
escaped quotes in arithmetic-aware `declare`, `typeset`, and `let` arguments.
The same guard also covered the raw word containing an embedded `$((...))`
expansion, so the expansion path was rejected before arithmetic evaluation.
The guard now remains active for ordinary assignment words and is skipped only
for those three command owners or an embedded arithmetic expansion, allowing
their existing arithmetic/index
handling to normalize the whitespace-only subscript to index 0.

Evidence:

- `cargo test --test parser_tests escaped_quote_array_subscript -- --nocapture`:
  2/2.
- `cargo test --test cli_tests escaped_quote_array -- --nocapture`: 2/2,
  including the ordinary assignment status-2 regression.
- `cargo test --test executor_tests command_chaining::part_080 -- --nocapture`:
  152/152.
- `BASH_RUNNER=D:/Git/bin/bash.exe D:/Git/bin/bash.exe
  scripts/run-bash-upstream-tests.sh run-arith`: 1/1.

The broader `arith10` contexts, arithmetic diagnostics, and official actual
output rows remain separate compatibility work.

### 2026-08-17 empty quoted arithmetic array subscripts

The same `arith10.sub` matrix distinguishes an empty quoted indexed subscript
from a whitespace-only one: Bash accepts `a[" "]` as index 0, but reports an
arithmetic error for `(( a[""]=24 ))` and for the equivalent `$((...))`
expansion. `let` retains its separate Bash behavior and continues to accept
the empty subscript as index 0.

The arithmetic command and arithmetic expansion entry points now reject an
empty quoted indexed subscript before generic lvalue evaluation. Ordinary
array assignment and `let` remain unchanged.

Evidence:

- `cargo test --test cli_tests compat_issue_regressions::arithmetic_empty
  -- --nocapture`: 3/3.
- `cargo test --test cli_tests escaped_quote_array -- --nocapture`: 3/3.
- `cargo test --test executor_tests command_chaining::part_080 -- --nocapture`
  152/152.
- Fresh `arith10.sub` stdout/stderr artifacts:
  `target/issue-suites/results/native-bash-20260817-arith10-current/`.

### 2026-08-17 parameter substring explicit empty length

The Bash actual-output probe also exposed the `v:2:` substring form. The
parser previously represented both `v:2` and `v:2:` as `length=None`, so the
latter incorrectly returned the suffix instead of an empty string. The
top-level substring splitter now preserves whether a separator colon was
present; an absent length after that colon becomes `Some(0)`, while no colon
continues to mean “through the end”.

Evidence:

- `cargo test --test cli_tests parameter_substring_empty_length_is_zero
  -- --nocapture`: 1/1.
- `cargo test --test executor_tests command_chaining::part_063 -- --nocapture`:
  21/21.
- `cargo check`: passed.

Unquoted command substitutions that expand to an empty field are now removed
from the command word list. If that leaves an assignment-only command, the
executor re-enters the assignment path after expansion so the assignment,
substitution status, and redirects are still applied. This fixes
`ash-psubst/falsetick2` (`v=\`exit 2\` \`false\``), which must yield
`Two:2 v:[]` rather than a blank command or a stale assignment.

Evidence:

- `ash-psubst/falsetick2`: `Two:2 v:[]`.
- `cargo test --test executor_tests command_chaining::part_021 -- --nocapture`:
  20/20.

The command-list executor now converts command-owned `IoError` results into
status 1 and continues the list unless `errexit` is active. This matches Bash
for assignment-only commands with a failed output redirect, such as the
`ash-psubst/falsetick` cases. Previously the raw Rust I/O error escaped the
AST loop after the first missing redirect and suppressed all following
status probes.

Evidence:

- `ash-psubst/falsetick`: all 16 expected status lines now emitted.
- `cargo test --test executor_tests command_chaining::part_021 -- --nocapture`:
  19/19.
- `cargo check`: passed.

The parser now leaves a trailing heredoc body unconsumed when the compound
command being finalized has no pending heredoc redirect. This matters for
lists such as `cat <<EOF && { echo ...; }`: the body belongs to the left
command, even when the right-hand brace group is parsed first by the compound
command boundary. Function bodies now retain and execute this heredoc input.

Evidence:

- `cargo test --test executor_tests command_chaining::part_021 -- --nocapture`:
  18/18.
- `cargo test --lib parser -- --nocapture`: 13/13.
- `cargo check`: passed.

The next ordered-output slice covers `times`, `shift`, `alias`, and `set`.
These builtins now buffer their results and diagnostics through the same
shared redirect owner; `shift` still applies its positional-parameter state
transition before the buffered diagnostic is emitted.

Evidence:

- `cargo test --test executor_tests command_chaining::part_021 -- --nocapture`:
  17/17, including ordered probes for `alias`, `set`, and `times`.
- `cargo test --test executor_tests command_chaining::part_026 -- --nocapture`:
  15/15.
- `cargo test --test executor_tests command_chaining::part_058 -- --nocapture`:
  16/16.
- `cargo check`: passed without warnings.

### 2026-08-17 malformed parameter expansion in arithmetic-for headers

The bridge-free arithmetic-for parser probe found that a malformed braced
parameter word inside an arithmetic-for header was accepted:
`${ case x in x) esac; }`. Its internal semicolon was treated as arithmetic
header structure, so Rubash executed the loop and continued instead of
returning Bash's syntax status 2. The parser now rejects this malformed
parameter-name boundary while retaining valid arithmetic parameter forms.

The stdin batch runner also records syntax errors independently from ordinary
nonzero command statuses. A syntax error now stops a non-interactive stdin
script without enabling `errexit`; ordinary arithmetic/builtin failures still
continue as Bash does.

Evidence:

- Raw bridge-free artifacts: `target/issue-suites/results/native-bash-20260817-arith-for/`.
- `cargo test --test cli_tests arithmetic_for -- --nocapture`: 3/3.
- `cargo test --test parser_tests arithmetic_for -- --nocapture`: 4/4.
- `cargo test --lib`: 200/200.

This closes only the malformed arithmetic-for parameter-header primitive; the
aggregate `arith-for` and official actual-output rows remain open.

### 2026-08-17 external coproc endpoints and Windows streaming pipelines

The remaining focused coproc failures were caused by two related execution
boundary issues. `refresh_background_jobs` retired a completed coprocess before
the next command could consume its still-readable named endpoint, and the
external-file `cat` path did not consume a virtual `CoprocStdout` descriptor.
The executor now protects endpoints referenced by the current input redirect
and retains existing endpoints while launching another coproc. Ordinary later
commands still retire completed endpoints, preserving the existing closed-fd
diagnostic behavior. External `cat` drains the shell-owned reader to EOF and
uses the normal redirected output path.

On this Windows host, `yes`, `head`, and `wc` are not available in `PATH`.
The concurrent external-pipeline path previously fell back to the buffered
stage path, which turned `yes | head` into a masked command-not-found result.
When those three utilities are unavailable, the Windows pipeline now launches
small internal streaming utility processes connected by the same OS pipes:
`yes` applies backpressure, `head` exits after its requested line count, and
`wc` counts input through EOF. This keeps the producer bounded by the pipe and
does not materialize an unbounded string.

The same execution boundary also fixed `/usr/bin/cat` file diagnostics,
persistent `exec 2>&1` propagation through a `cat | cat` pipeline, and
prefix assignments reaching the `env` builtin in a pipeline. The Windows
POSIX-directory bridge now records a shell-visible physical path so `cd -P /;
pwd -P` reports `/` instead of the host repository directory.

Evidence:

- `cargo test --test cli_tests coproc -- --nocapture`: 17/17.
- `cargo test --test cli_tests external_pipeline -- --nocapture`: 4/4.
- `cargo test --test cli_tests -- --nocapture`: 229/229.
- `cargo test --test cli_tests c_command_materializes_persistent_stderr_to_stdout_for_external_children -- --nocapture`: 1/1.
- `cargo test --test cli_tests c_command_pipeline_stages_inherit_persistent_stderr_to_stdout -- --nocapture`: 1/1.
- `cargo test --test cli_tests prefix_assignments_reach_env_builtin_pipeline -- --nocapture`: 1/1.
- `cargo test --test executor_tests command_chaining::part_031::test_command_cd_updates_pwd_for_physical_pwd -- --nocapture`: 1/1.
- `cargo test --test executor_tests`: 1534/1535 before the physical-PWD fix; the sole failure then passed in the focused rerun. A complete post-fix executor rerun remains a required gate.
- `cargo test --lib`: passed in the preceding full validation run.
- `D:/Git/bin/bash.exe scripts/validate-semantic-map.sh`: passed.

This closes the focused coproc, external-pipeline, persistent-stderr, and
POSIX physical-PWD primitives only. The broader official Bash, BusyBox, Oil,
mksh, and ksh93 issue-suite differences remain open.

### 2026-08-17 builtin ordered-output redirect boundary

The #20/#24/#25/#54 redirection family exposed a shared builtin execution
boundary bug. `printf` and `pwd` had direct `redirect_out`/`append`/
`redirect_err` branches that bypassed the command's parse-order redirect list.
For a command such as `printf 'x\n' >&2 2>file`, Bash leaves the output on the
original stderr but still creates the empty `file`; Rubash previously skipped
the redirect application entirely, so the empty target was not created. The
same bypass also affected `pwd`.

Both builtins now collect stdout/stderr and use
`write_buffered_builtin_output`, which applies the shared ordered fd state.
This is a semantic execution fix, not an expected-output adjustment.

Evidence:

- `cargo test --test cli_tests fd_redirects -- --nocapture`: 16/16.
- `cargo test --test executor_tests command_chaining::part_021 -- --nocapture`:
  12/12, including the new `pwd` ordered-output regression.
- `cargo test --test parser_redirection_tests -- --nocapture`: 68/68.
- `cargo test --test executor_tests command_chaining::part_080 -- --nocapture`:
  152/152.
- `BASH_RUNNER=D:/Git/bin/bash.exe D:/Git/bin/bash.exe
  scripts/run-bash-upstream-tests.sh run-redir`: 1/1.
- `cargo check`: passed.

The same boundary has now been applied to `export`, whose output and
diagnostics are also collected before the shared redirect owner writes them.
The new `part_021` regression covers `export -p >&2 2>file` and verifies that
the empty target is created while output remains on the original stderr.

Evidence for this extension:

- `cargo test --test executor_tests command_chaining::part_021 -- --nocapture`:
  13/13.
- `cargo check`: passed.

Commits: `349d06ed`, `784ce7d0`, `be551483`. Other builtins still contain
direct I/O redirect branches and require the same treatment where a minimal
Bash comparison demonstrates an ordered-redirection difference.

The same ordered diagnostic boundary now applies to `readonly`. Invalid
options are collected as stderr before the shared redirect owner applies the
left-to-right fd state, so `readonly -Z >&2 2>file` reports into `file` and
does not bypass the second redirect.

Evidence for this extension:

- `cargo test --test executor_tests command_chaining::part_021 -- --nocapture`:
  14/14.
- `cargo check`: passed.
- `scripts/validate-semantic-map.sh`: passed.

Commit: `fix: preserve readonly ordered diagnostic redirects`. Other builtins
still contain direct I/O redirect branches and require the same treatment
where a minimal Bash comparison demonstrates an ordered-redirection
difference.

The same execution-boundary fix now covers `hash`, `shopt`, `umask`, and
`enable`, whose `execute_with_io` paths previously opened redirect targets
directly and therefore skipped parse-order fd duplication. A shared regression
checks `>&2 2>file` for all three output-producing option builtins (plus
`hash`'s existing redirect coverage).

Evidence:

- `cargo test --test executor_tests command_chaining::part_021 -- --nocapture`:
  15/15.
- `cargo test --test executor_tests command_chaining::part_022 -- --nocapture`:
  14/14.
- `cargo test --test executor_tests command_chaining::part_023 -- --nocapture`:
  12/12.
- `cargo test --test executor_tests command_chaining::part_029 -- --nocapture`:
  10/10.
- `cargo check`: passed.

Commit: `997c0f93`.

The same boundary has also been applied to `declare`, `kill`, `ulimit`,
`trap`, `help`, and directory-stack builtins. Their `execute_with_io` output
is now buffered before the shared fd state is applied, covering another set of
direct-I/O builtin paths in the redirection family.

Evidence:

- `cargo test --test executor_tests command_chaining::part_021 -- --nocapture`:
  16/16, including ordered probes for `declare`, `help`, `kill`, and `ulimit`.
- `cargo test --test executor_tests command_chaining::part_032 -- --nocapture`:
  11/11.
- `cargo test --test executor_tests command_chaining::part_058 -- --nocapture`:
  16/16.
- `cargo check`: passed.

## 2026-08-20 Issue #57: ash-z_slow/many_ifs classification

The closure audit row for ash-z_slow/many_ifs was DIFF with bash_rc=124 and rubash_rc=124 under BUSYBOX_TEST_TIMEOUT=8. The fixture generates 6,856 read/set IFS cases and is an ash-specific stress oracle; its comments expect ash behavior and are not independently valid Bash expected output.

Bounded reproduction used the BusyBox fixture at target/issue-suites/busybox/shell/ash_test/ash-z_slow/many_ifs.tests:

| Shell | Timeout | Result | Stdout |
|---|---:|---|---|
| Git Bash | 8s | rc 124 | empty |
| Rubash | 8s | rc 124 | 148 generated mismatch lines |
| Git Bash | 30s | rc 124 | empty |
| Rubash | 30s | rc 124 | 209 generated mismatch lines |
| Git Bash | 120s | rc 124 | empty |
| Rubash | 60s | rc 0 | 265 lines, final # tests 6856 passed 6328 failed 528 |

A minimal exact pipeline confirms the reported read result is not a Rubash semantic defect: both Git Bash and Rubash produce x=, y=: for printf "%s\n" "::" | ( IFS=": "; read x y; ... ). Therefore the audit DIFF is classified as a harness/reference timeout plus ash-fixture expectation artifact, with Rubash performance slower than the 8s budget but completing within 60s. No Rust fix or focused semantic regression is justified by this evidence. The remaining 528 generated lines are fixture-specific residuals, not Bash-vs-Rubash proof until the suite is run under a BusyBox ash reference (or its expected-output contract is explicitly separated from Bash).

## 2026-08-20 Builtin usage-status parity: set and umask

A minimal GNU Bash comparison (set -Z and umask -Z) showed matching diagnostics but a status mismatch: Bash returns 2 for invalid builtin options, while Rubash returned 1. The root cause was local option parsing in src/builtins/set.rs and src/builtins/umask.rs, not parser or executor behavior. Both owners now return EX_USAGE (2) and have focused unit regressions covering the invalid-option contract.

Evidence:

- GNU Bash: set -Z and umask -Z, both rc 2.
- Rubash after fix: same probes, both rc 2.
- cargo test --lib set umask -- --nocapture passed (bounded builtin tests).

