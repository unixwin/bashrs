# Bash Actual-Output DIFF TODO Ledger

> Snapshot date: 2026-08-13 14:02:08
> Repository: `D:/repo/rubash`
> Source result: `target/issue-suites/results/bash-actual/results.tsv`
> Current repository HEAD at review time: `88f72fc`

This is the per-test TODO ledger for the GNU Bash `.tests` actual-output
comparison. It is intentionally separate from the higher-level compatibility
plan. A row below is not considered closed merely because a related `.right`
runner reports PASS.

## Snapshot

The stored snapshot contains 83 Bash test files:

```text
TOTAL=83 PASS=15 DIFF=68
```

The 68 DIFF rows are grouped by the recorded exit-code pair:

| Bash/Rubash status | Count | Initial interpretation |
|---|---:|---|
| `0/0` | 37 | Both complete; stdout differs. Usually a semantic or path/output difference, but still needs a minimal probe. |
| `1/0` | 2 | Rubash loses an arithmetic or runtime failure status. |
| `2/0` | 11 | Bash rejects syntax or builtin usage; Rubash accepts it or does not propagate the parse status. |
| `9/0` | 1 | Termination or signal status differs; isolate host process behavior first. |
| `124/0` | 5 | Bash-side timeout in this replicated runner; determine whether this is a Bash test harness, host path, or Rubash hang. |
| `127/0` | 12 | Command lookup or test environment mismatch is possible; do not classify as a Rubash semantic failure before reproducing the body outside the runner. |

## Evidence Limits

The current runner is `target/issue-suites/run-bash-actual-difftest.sh`.
It stores both stdout and stderr under each test's `work/<test>/bash/` and
`work/<test>/rubash/` directories, but the PASS/DIFF decision currently
compares only exit status and stdout. The generated `diffs.txt` also keeps
only the first 120 lines of each stdout diff. Therefore:

- the raw per-test stdout and stderr files are the evidence;
- `results.tsv` is a snapshot, not a history database;
- stderr differences are diagnostic evidence but are not currently part of the
  PASS/DIFF predicate;
- the snapshot predates the current `88f72fc` HEAD and must be refreshed
  before claiming that a TODO is fixed.

Before closing any row, record a new artifact directory with the tested
commit, for example:
`target/issue-suites/results/bash-actual-<commit>/`.

## Cross-Cutting TODOs

- [ ] Decide and document whether Bash `.tests` compatibility includes stderr.
      If it does, update the runner to compare stderr as well as stdout. If it
      does not, keep stderr diagnostic-only and state that explicitly in the
      result format.
- [ ] Make the runner preserve dated result directories instead of deleting
      the previous `work/` tree and overwriting the only summary table.
- [ ] Record the tested Rubash commit, GNU Bash executable, timeout value, and
      host path mode in every result directory.
- [ ] Re-run the complete bounded actual-output set after the current fd/job
      changes, then update this ledger rather than treating the 2026-08-13
      snapshot as current.
- [ ] For every `124/0`, reproduce the individual `.tests` body with a per-file
      timeout and save process cleanup evidence. A timeout is not a root-cause
      classification by itself.
- [ ] For every `127/0` and `9/0`, run the body with the same command lookup,
      working directory, and environment under GNU Bash and Rubash before
      changing semantic code.

## 2026-08-14 Jobs Progress

The first `jobs`/`wait -n` root-cause slice is implemented. The old behavior
only registered children in `JobTable`; `jobs` never polled the live
`Child` handles, and `wait -n` selected the first registered running child.
That made an already completed later job wait behind an earlier slow job and
made `jobs -l` report `Running` after completion.

The current fix in `src/executor/job_builtins.rs` now:

- calls nonblocking `try_wait` before `jobs` and `wait`;
- records completed statuses in `JobTable` and the explicit-wait retention
  adapter;
- makes `wait -n` prefer a completed job that it has not consumed yet;
- renders completed jobs as `Done` or `Exit N`;
- removes all compatibility tracking when an explicit wait consumes a status,
  while preserving the status needed by a later explicit `wait PID` after
  `wait -n`.

Raw probe artifacts:
`target/issue-suites/results/jobs-kernel-20260814/`

Verification:

- `cargo test --test executor_tests command_chaining::part_036 -- --nocapture`
  -> 29/29 passed;
- `cargo test --lib jobs` -> 2/2 passed;
- Bash and Rubash `jobs -l` probe both report the completed `false` job as
  `Exit 1`;
- Bash and Rubash out-of-order `wait -n` probe both report `status:1`;
- bounded `run-jobs` -> 1/1 passed, with log at
  `target/bash-upstream-tests/logs/run-jobs.log`.

This does not close the `jobs` row. Remaining work is to make `JobTable` the
only job-state source, handle no-argument `wait` and pipeline aggregate
statuses consistently, and close coproc/process-substitution endpoints when
the corresponding job is consumed. The stored 2026-08-13 official
actual-output `jobs` row must be refreshed against the current commit before
the row can be marked complete.

## 2026-08-14 Coproc Progress

The virtual descriptor close path now has a real regression for a named
coprocess. `exec {C[1]}>&-` closes only the coprocess stdin writer capability;
it does not invalidate `C[0]`, so a Rubash coprocess loop observes EOF after
the writer is closed. The owner is
`src/executor/trap_exec.rs::close_dynamic_output_fd`, with the old environment
fd keys retained only as a compatibility mirror.

Verification:

- `cargo test --test cli_tests c_command_closing_named_coproc_stdin_fd_produces_eof -- --nocapture`
  -> 1/1 passed.
- `cargo test --test executor_tests command_chaining::part_080 -- --nocapture`
  -> 152/152 passed, including named coprocess, fd, and process-substitution
  regressions.
- `BASH_RUNNER=D:/Git/bin/bash.exe D:/Git/bin/bash.exe scripts/run-bash-upstream-tests.sh run-coproc`
  -> 1/1 passed; raw log:
  `target/bash-upstream-tests/logs/run-coproc.log`.
- Direct external-child probe artifact:
  `target/issue-suites/results/coproc-kernel-20260814/`.
  GNU Bash produced `read:hello\ndone\n`; Rubash produced
  `read:\ndone\n` for `coproc { cat; }`. Both exited after the bounded probe.

The `coproc` row remains open. The passing Rust probe uses a Rubash loop and
proves virtual writer-close/reader-preservation semantics. It does not prove
that an external child such as `cat` receives the coprocess writer's EOF after
fd materialization. The remaining TODOs are:

- [ ] Add a minimal external-child regression for `cat <&"${C[0]}"` and
      `cat >&"${C[1]}"`, with Bash/Rubash stdout, stderr, status, and timeout
      artifacts saved under a dated result directory.
- [ ] Trace `FdTable::materialize_for_child` and the WinuxCmd/std child setup
      boundary to ensure the materialized coprocess stdin handle is closed when
      the shell closes `C[1]`.
- [ ] Verify external-child EOF, `wait "$C_PID"` status, duplicate fd close,
      and reader-close behavior against GNU Bash before changing the row.
- [ ] Refresh the official `.tests` `coproc` row against the current commit;
      do not close it from the `run-coproc` `.right` result.

## Per-Test TODOs

Status labels used below:

- `semantic lead`: the result already points to a Rubash subsystem, but still
  needs a minimal reproducer and a focused regression;
- `isolate first`: the exit status or environment makes a semantic conclusion
  unsafe;
- `deferred/host`: likely interactive, locale, or host integration work;
- `unclassified`: the family is known, but this test has no durable per-test
  root-cause note yet.

| Test | Bash/Rubash | Current classification | TODO |
|---|---:|---|---|
| `alias` | `0/0` | semantic lead: alias expansion timing, quoted aliases, listing, and invalid alias diagnostics | [ ] Split the failing body into alias expansion, `alias -p`, quoted use, and `unalias`; add focused regressions. |
| `arith-for` | `2/0` | semantic lead: arithmetic-for parsing and arithmetic error propagation | [ ] Reproduce each malformed arithmetic-for and invalid operand; make parser/status match Bash; add parser and executor tests. |
| `arith` | `1/0` | semantic lead: arithmetic diagnostics, invalid constants, division by zero, lvalue and recursion errors | [ ] Compare each remaining `arith*.sub` primitive; propagate Bash status and diagnostics without changing already passing arithmetic cases. |
| `array` | `1/0` | semantic lead: array syntax, bad subscripts, readonly arrays, sparse indexes, and array assignment status | [ ] Isolate syntax rejection, subscript validation, readonly behavior, and assignment status; add array regressions. |
| `assoc` | `0/0` | semantic lead: associative assignment/subscript rules and word expansion | [ ] Compare one associative-array operation at a time, including empty subscripts, readonly/unset, quoting, and path-shaped values. |
| `attr` | `0/0` | semantic lead: readonly/export/integer and array attribute diagnostics | [ ] Align attribute mutation errors and output; add `declare`/`readonly` focused tests. |
| `braces` | `0/0` | semantic lead: brace expansion edge cases and malformed command substitution inside the test | [ ] Extract each differing brace form and command-substitution case; verify parser and expansion ownership. |
| `builtins` | `2/0` | mixed builtin option, syntax, diagnostic, and status differences | [ ] Split the large file by builtin and create one TODO/test per failing builtin; do not patch aggregate output. |
| `case` | `0/0` | semantic lead: case pattern/grammar and nested command behavior | [ ] Extract the first differing case form and compare parser tokens, pattern expansion, and final status. |
| `complete` | `2/0` | deferred/host candidate: programmable completion is interactive-facing | [ ] Confirm which body is noninteractive; classify the rest as host-owned or implement a real completion owner and test gate. |
| `comsub-eof` | `0/0` | semantic lead: command-substitution EOF and heredoc collection | [ ] Re-run the remaining body after current heredoc fixes; isolate parser EOF from execution output. |
| `comsub-posix` | `0/0` | semantic lead: POSIX command-substitution parsing/status | [ ] Compare POSIX-mode cases individually and add status/EOF regressions. |
| `comsub` | `0/0` | semantic lead: command-substitution state isolation and expansion | [ ] Identify the first differing command substitution and test variable, stdin, status, and nested substitution state separately. |
| `comsub2` | `2/0` | unclassified parser/command-substitution status difference | [ ] Reproduce the exact malformed source; determine whether parser EOF, nested compound parsing, or status propagation is responsible. |
| `cond` | `0/0` | semantic lead: conditional grammar, arithmetic operands, and test status | [ ] Split `[[ ]]`, arithmetic, pattern, and invalid-operator cases; add one parser/status regression for each. |
| `coproc` | `0/0` | partial semantic fix: virtual writer close and reader preservation are covered; external-child materialization remains open | [x] Rubash coproc loop observes EOF after `exec {C[1]}>&-`. [ ] Verify external child fd materialization, EOF, reader close, wait status, and refresh the official actual-output row. |
| `dbg-support` | `0/0` | semantic lead: DEBUG/RETURN/EXIT trap metadata and `BASH_COMMAND` state | [ ] Compare hook order and `FUNCNAME`/`BASH_COMMAND` values; add focused trap regressions. |
| `dstack` | `0/0` | unclassified: directory-stack state and builtin output | [ ] Extract `pushd`, `popd`, `dirs`, and invalid-directory cases; assign ownership between shell state and builtins. |
| `dynvar` | `0/0` | unclassified: dynamic variable scope and assignment state | [ ] Reproduce the first dynamic-scope mismatch; compare nameref, function scope, and command-substitution state. |
| `errors` | `2/0` | semantic lead: syntax/error propagation | [ ] Group malformed parser, builtin, and redirection cases; require Bash-compatible rc=2 or diagnostic before closing. |
| `exp` | `0/0` | semantic lead: general word/parameter expansion | [ ] Bisect the body into parameter expansion, splitting, quote removal, and command substitution; add the smallest failing regression. |
| `exportfunc` | `127/0` | isolate first: environment/function export or missing command in replicated harness | [ ] Run the body outside the copied test directory with traced command lookup; classify environment noise before touching export semantics. |
| `extglob` | `0/0` | semantic lead: extglob parsing and pathname expansion | [ ] Separate valid extglob matching from invalid syntax and filesystem-path cases; add parser and glob regressions. |
| `func` | `0/0` | semantic lead: function arguments, scope, return status, and traps | [ ] Find the first differing function body; compare positional parameters, locals, nested calls, and status propagation. |
| `getopts` | `0/0` | semantic lead: option parsing state and diagnostics | [ ] Compare `OPTIND`, missing arguments, silent mode, and repeated calls; add builtin tests. |
| `glob-bracket` | `2/0` | semantic lead: invalid bracket pattern parsing/status | [ ] Match Bash rejection and diagnostic for malformed bracket expressions; keep valid Windows path patterns covered. |
| `glob` | `0/0` | semantic lead: pathname expansion and Windows separator conversion | [ ] Separate no-match, escaped separators, ordering, and path normalization; add filesystem-isolated tests. |
| `globstar` | `0/0` | semantic lead: recursive globstar traversal and path display | [ ] Build a temporary fixture tree and compare `globstar` results, ordering, and separators. |
| `heredoc` | `0/0` | semantic lead: delimiter collection, quoting, expansion, and delivery | [ ] Re-run the remaining body with bounded timeout; isolate delimiter parsing, command substitution, and large-input delivery. |
| `histexp` | `2/0` | deferred/host candidate: history expansion is interactive-sensitive | [ ] Confirm whether this body requires interactive history; classify host-owned behavior or add a noninteractive history owner. |
| `history` | `0/0` | deferred/host candidate: history storage and display | [ ] Separate interactive history behavior from shell semantic cases; document non-targets and test any retained target. |
| `ifs-posix` | `124/0` | isolate first: POSIX IFS body timed out under the runner | [ ] Run the file with a per-test timeout, locate the blocking command, and compare stdin ownership before changing the splitter. |
| `ifs` | `0/0` | semantic lead: empty fields, delimiters, quoting, and POSIX splitting | [ ] Extract the first mismatch and add focused word-splitting regressions for unquoted, quoted, `$*`, and `$@`. |
| `intl` | `0/0` | deferred/host candidate: locale/gettext environment behavior | [ ] Determine whether the difference is locale availability or Bash semantic output; record host-owned scope if it is not a Rubash target. |
| `invocation` | `0/0` | semantic/host boundary: startup mode, environment, and script invocation | [ ] Re-run from identical working directories and environment; separate Rubash invocation semantics from Windows path lookup. |
| `iquote` | `127/0` | isolate first: quote test invokes a missing command or harness path | [ ] Trace the missing command and working directory; only classify quoting after the body executes under both shells. |
| `jobs` | `124/0` | partial semantic fix: refresh and out-of-order `wait -n` now have real coverage; no-argument wait, pipeline state, and endpoint cleanup remain | [x] Nonblocking refresh, completed `jobs -l`, and completed-first `wait -n` are covered. [ ] Make `JobTable` the only state source, finish no-argument wait/pipeline status/coprocess cleanup, then refresh the official actual-output row. |
| `mapfile` | `0/0` | semantic lead: fd input, array storage, delimiters, and status | [ ] Compare `-u`, callbacks, delimiters, empty input, and array indexes separately; add executor tests. |
| `more-exp` | `127/0` | isolate first: expansion body depends on command/path availability | [ ] Trace the missing command and rerun the body independently; then split parameter, arithmetic, and command substitution expansion. |
| `nameref` | `127/0` | isolate first: nameref body has command/environment mismatch in this runner | [ ] Reproduce with a minimal script not invoking the copied test harness; then test nameref assignment, unset, array, and readonly behavior. |
| `new-exp` | `127/0` | isolate first: command lookup/environment mismatch likely | [ ] Identify the missing Bash-side command and rerun only the expansion primitives before assigning a semantic owner. |
| `nquote` | `0/0` | semantic lead: quote removal and word splitting | [ ] Extract the first quote-removal mismatch and add regression coverage for escaped spaces, empty fields, and nested substitutions. |
| `nquote1` | `127/0` | isolate first: environment/command lookup may mask quoting behavior | [ ] Make the test body self-contained, rerun both shells, then classify quote-removal differences. |
| `nquote2` | `127/0` | isolate first: environment/command lookup may mask quoting behavior | [ ] Same isolation procedure; preserve Bash stderr and status as diagnostic evidence. |
| `nquote3` | `127/0` | isolate first: environment/command lookup may mask quoting behavior | [ ] Same isolation procedure; add a focused quoting regression only after a reproducible semantic mismatch. |
| `nquote4` | `127/0` | isolate first: environment/command lookup may mask quoting behavior | [ ] Same isolation procedure; distinguish path conversion from quote semantics. |
| `nquote5` | `0/0` | semantic lead: advanced quote/expansion interaction | [ ] Extract the first mismatch and compare quote removal, arrays, and command substitution independently. |
| `parser` | `2/0` | semantic lead: permissive grammar and rc=2 propagation | [ ] Enumerate each malformed construct still accepted by Rubash; add parser tests and preserve valid newline cases. |
| `posix2` | `9/0` | isolate first: termination/status mismatch | [ ] Reproduce with process tracing and timeout cleanup; determine whether the status comes from a host process, test harness, or Rubash. |
| `posixexp` | `2/0` | semantic lead: POSIX expansion/parser error status | [ ] Split invalid syntax from valid POSIX expansion; align diagnostics and rc=2 propagation. |
| `posixpipe` | `0/0` | semantic lead: pipeline stdin/fd lifecycle and POSIX status | [ ] Compare pipeline member status, `PIPESTATUS`, `pipefail`, and stdin closure in a minimal fixture. |
| `printf` | `2/0` | semantic lead: builtin format/argument parsing and status | [ ] Separate invalid format, missing argument, numeric conversion, and escape behavior; add builtin tests. |
| `procsub` | `124/0` | semantic lead plus host boundary: process-substitution fd and path lifecycle | [ ] Run each `<(...)`/`>(...)` primitive with timeout; verify endpoint ownership, child completion, path translation, and cleanup. |
| `quote` | `127/0` | isolate first: quote body likely affected by command lookup/environment | [ ] Rerun self-contained quoting cases; only then assign expansion fixes. |
| `quotearray` | `0/0` | semantic lead: quoted indexed/associative array expansion | [ ] Compare `${array[@]}`, `${array[*]}`, empty members, and nested quotes; add expansion regressions. |
| `read` | `124/0` | semantic lead plus host boundary: redirected stdin/fd EOF and read loop | [ ] Run each `read` case with timeout; inspect fd ownership, heredoc/input precedence, close behavior, and EOF. |
| `redir` | `124/0` | semantic lead plus host boundary: ordered redirection and fd lifetime | [ ] Split ordered dup/close, `/dev/null`, dynamic fd, heredoc, and external-child cases; fix the shared fd owner and add regressions. |
| `rhs-exp` | `0/0` | semantic lead: parameter substitution RHS expansion and quote rules | [ ] Extract pattern replacement, backslash, empty RHS, and command-substitution cases; add parameter-expansion tests. |
| `rsh` | `0/0` | host/path boundary candidate: restricted-shell invocation and command lookup | [ ] Reproduce with identical PATH and working directory; separate restricted-shell semantics from Windows command availability. |
| `set-e` | `0/0` | semantic lead: errexit suppression and status propagation | [ ] Compare pipelines, conditionals, command substitutions, functions, and subshell contexts; add one regression per suppression rule. |
| `set-x` | `0/0` | semantic lead: tracing output and command expansion state | [ ] Compare PS4, quoting, redirections, functions, and command substitutions; define whether stderr trace output is in the contract. |
| `shopt` | `127/0` | isolate first: command lookup/environment mismatch; builtin parity still needs coverage | [ ] Rerun the body self-contained, then split option query, mutation, `-o`, and unsupported-option status. |
| `test` | `0/0` | semantic lead: test expression parsing, operators, and status | [ ] Extract string, numeric, file, and compound expression differences; add builtin tests without hardcoding output. |
| `tilde2` | `0/0` | semantic lead: tilde expansion in assignments, paths, and quoted contexts | [ ] Compare home, login, assignment, and path-shaped inputs under the Windows home mapping. |
| `trap` | `2/0` | semantic lead: trap option parsing, invalid signals, and status/diagnostic propagation | [ ] Split `-l`, `-p`, `-P`, action-only, invalid signal, and execution timing cases; add trap regressions. |
| `type` | `127/0` | isolate first: command lookup/PATH mismatch likely | [ ] Reproduce with a controlled PATH and command fixture; then align builtin, function, alias, and file classification. |
| `varenv` | `0/0` | semantic lead: exported environment, shell state, and child inheritance | [ ] Compare scalar/array export, special variables, subshell state, and child environment serialization. |
| `vredir` | `0/0` | semantic lead: dynamic fd allocation, ordered redirects, close/move, and diagnostics | [ ] Run `vredir4`, `vredir5`, `vredir7`, and `vredir8` primitives separately; connect each result to `FdTable` tests and remove only the corresponding bridge after real coverage. |

## Closure Rule

A row can be marked complete only when all of the following exist:

- a minimal Bash/Rubash reproducer with saved stdout, stderr, and status;
- a named Rust semantic owner;
- a focused Rust regression or an explicit host-owned/deferred decision;
- a bounded Bash actual-output or upstream test result after the change;
- a dated artifact tied to the tested commit;
- no `.right` output bridge is required to obtain the result.

Until then, the row remains an open TODO even if a focused `.right` runner
reports PASS.
