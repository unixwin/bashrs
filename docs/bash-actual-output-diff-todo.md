# Bash Actual-Output DIFF TODO Ledger

> Snapshot date: 2026-08-15 12:40:15 CST
> Repository: `D:/repo/rubash`
> Source result: `target/issue-suites/results/bash-actual/results.tsv`
> Current repository HEAD at review time: `88f72fc` (working tree had uncommitted changes)

This is the per-test TODO ledger for the GNU Bash `.tests` actual-output
comparison. It is intentionally separate from the higher-level compatibility
plan. A row below is not considered closed merely because a related `.right`
runner reports PASS. The required remediation phase order is recorded in
`docs/actual-output-diff-remediation-plan.md`; later phases must not start before
the current phase gate is closed.

## Snapshot

The stored snapshot contains 83 Bash test files:

```text
TOTAL=83 PASS=12 DIFF=71
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
- this snapshot was refreshed against the dirty working tree at `88f72fc`; it is
  not a clean-commit baseline and must not be used as a release compatibility
  score until host/path and upstream-bridge classifications are separated.

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

## 2026-08-14 Coproc And Background-Termination Follow-Up

The remaining official `coproc.tests` hang had two concrete semantic causes,
both now covered by real Rust code and regressions:

1. `cat -` was classified as an external file operand because the host-side
   cat adapter only streamed the no-operand form. In a coprocess child this
   delegated stdin to `cat.exe`, which could retain the nested pipe and block
   the parent `read`. `external_file_builtins.rs` now treats `-` as stdin when
   there are no file operands and streams inherited stdin through Rubash.
2. Windows `kill PID` sent SIGTERM to another Rubash process through its
   cooperative mailbox. A coprocess blocked in a native pipe read cannot poll
   that mailbox, so `wait $PID` never completed. `builtins/kill.rs` now keeps
   mailbox delivery for the current shell, while cross-process Windows jobs
   fall through to native process termination.

Real regressions:

- [x] `cargo test --test cli_tests c_command_external_cat_dash_receives_coproc_data_before_writer_close -- --nocapture`
- [x] `cargo test --test cli_tests c_command_starts_cat_dash_coproc_after_waiting_for_previous_coproc -- --nocapture`
- [x] `cargo test --test cli_tests coproc -- --nocapture` -> 14/14
- [x] Bridge-free official body now completes without timeout.

Raw comparison:
`target/issue-suites/results/coproc-actual-20260814-catdash-term/`

| Run | Status | Interpretation |
|---|---:|---|
| GNU Bash source body | `0` | Reference output; `REFLECT` is terminated with status 143. |
| Rubash source body | `1` | The coproc lifecycle output now completes; remaining output/status differences are environment or final fd-diagnostic work. |

Remaining TODOs for the `coproc` row:

- [ ] Provide a Windows `xcase` equivalent or classify the missing support
      utility as harness-owned; do not implement it as a coproc shortcut.
- [ ] Define the Windows mapping for `/etc/passwd` and compare the pipeline
      through an available fixture before judging pipeline semantics.
- [ ] Reproduce the final `exec 4<&${COPROC[0]}-; exec >&${COPROC[1]}-;
      read foo <&4` sequence in a minimal probe. Align closed-pipe behavior
      and status with Bash, or document the intentional Windows diagnostic.
- [ ] Refresh the official actual-output ledger row after the three remaining
      items are classified; keep the `coproc.tests` upstream `.right` bridge
      enabled until then.

## 2026-08-14 Coproc Streaming Child Progress

The bridge-free delayed external-child reproducer is now fixed. Before the
fix, `coproc C { cat; }` received `hello` through `C[1]`, but the parent read
from `C[0]` could not complete before the writer was closed because nested
Rubash delegated the no-operand inherited-stdin `cat` to `winuxcmd/cat.exe`.
The semantic owner now streams inherited process stdin in chunks from
`src/executor/external_file_builtins.rs`, flushing each chunk through the
existing Rubash output path. Explicit redirects such as `cat <&"${C[0]}"`
remain on the fd-table child setup path.

Raw artifact:
`target/issue-suites/results/coproc-stream-20260814-6d83e10/`.
Both Bash and Rubash returned rc 0 and exact stdout `read:hello\n`; the
reproducer filename deliberately does not match `coproc.tests`, so the
upstream output bridge cannot intercept it.

Verification:

- [x] `cargo test --test cli_tests c_command_external_cat_receives_coproc_data_before_writer_close -- --nocapture`
      passes with a 5-second child timeout.
- [x] `cargo test --test cli_tests coproc -- --nocapture` -> 10/10.
- [x] `cargo test --test executor_tests command_chaining::part_080 -- --nocapture`
      -> 152/152.
- [x] Add and verify the symmetric external writer case (`cat` output to
      `C[1]`) and duplicate/close lifetime cases.
- [x] Run the official body without the `coproc.tests` upstream bridge and
      save its Bash/Rubash outputs under the dated raw result directory.
- [ ] Classify `/etc/passwd`, `xcase`, and final closed-pipe diagnostics
      separately as host/harness or remaining semantic differences.
- [ ] Refresh the official actual-output ledger row only after the bridge-free
      body is green or every remaining difference is documented as
      environment-only.

## 2026-08-14 Coproc Lifecycle And Diagnostic Classification

The final closed-pipe reproducer is now implemented at the job/fd lifecycle
owner rather than in the `read` builtin. Before this change, a completed
coprocess left its virtual `COPROC[0]`/`COPROC[1]` endpoints and the legacy fd
environment mirror alive. A later `exec 4<&${C[0]}-; read foo <&4` therefore
looked like an ordinary EOF. Bash retires the coprocess variables when the
child exits, and the moved fd reports `4: Bad file descriptor`.

`execute_ast_inner` now refreshes background jobs before each command.
Completed coprocesses are retired by `job_builtins.rs`: endpoint aliases are
closed in `FdTable`, compatibility fd keys are removed, and the coprocess
array plus `${NAME}_PID` are unset. Redirected numeric fds are validated by
the `read` owner only after this state transition. This preserves normal EOF
for a live/open stream and reports a closed descriptor for a retired one.

Focused evidence:

- [x] `cargo test --test cli_tests coproc -- --nocapture` -> 15/15.
- [x] `cargo test --test cli_tests c_command_retires_finished_coproc_endpoints_before_later_redirects -- --nocapture` -> 1/1.
- [x] `cargo test --test executor_tests command_chaining::part_080 -- --nocapture` -> 152/152.
- [x] EOF after a coproc reader reaches the end no longer falls back to
      `__RUBASH_COPROC_STDIN:<pid>`.
- [x] BrokenPipe from a coproc writer is rendered as
      `command: write error: Bad file descriptor`, without Windows error 232.
- [x] Cross-process Windows SIGTERM now produces status `143` for the
      `REFLECT` coprocess regression.

Latest bridge-free official comparison:
`target/issue-suites/results/coproc-actual-20260814-coproc-fd2/`

| Run | Status | Relevant result |
|---|---:|---|
| GNU Bash source body copied to `coproc-actual-body.sh` | `0` | `REFLECT` status `143`; `/etc/passwd` and closed-fd diagnostics appear on stdout; coproc array displays Bash fd values `63 60`. |
| Rubash source body copied to `coproc-actual-body.sh` | `0` | `REFLECT` status `143`; pipeline `/etc/passwd` and `xcase` diagnostics now appear on stdout; coproc array displays virtual PIDs. |

Remaining TODOs are deliberately split by ownership:

- [ ] `xcase` is absent on this Windows host. Classify it as a
      harness/host-owned missing utility or provide a real Windows fixture;
      do not add a coproc-specific shortcut.
- [ ] `/etc/passwd` is not present on the Windows host. Keep this as a
      fixture/path TODO. The direct external-child case is fixed and covered,
      and the concurrent external pipeline now routes intermediate-stage fd2
      through the persistent shell fd2 owner. The missing path itself remains
      a host fixture decision.
- [x] Add and pass a focused regression for `exec 2>&1` followed by an
      external child that fails to open a path; its diagnostic is now on
      stdout and status remains 1.
- [x] Add and pass the corresponding pipeline-stage regression without using
      the simple `cat` stdin shortcut; intermediate stderr is drained without
      blocking and written through the persistent fd2 owner.
- [x] Classify coproc-child stderr separately: `coproc xcase -n -u` is a
      missing host command, and its command-not-found diagnostic now follows
      the parent's fd2 endpoint in the bridge-free body.
- [x] Give coprocess children an explicit fd2 materialization path. The
      `compound_exec.rs` owner now forwards piped child stderr through a
      snapshot of the parent `FdTable` endpoint and joins the forwarder when
      the child completes. Explicit coproc stderr redirects still override
      this default path.
- [ ] Replace the coproc array's `(pid pid)` virtual values with stable shell
      fd values or document a deliberate Windows representation. Bash prints
      `63 60`; Rubash currently prints the child PID twice. This is observable
      even when endpoint reads/writes work.
- [ ] Match the diagnostic produced by `echo ${COPROC[@]}` after
      `exec >&${COPROC[1]}-`. Bash reports `echo: write error: Bad file
      descriptor`; Rubash currently emits an empty line and only reports the
      later `4: Bad file descriptor` from `read <&4`.
- [ ] Refresh the official actual-output ledger only after the `xcase`, path
      fixture, coproc-child fd2, virtual-fd display, and closed-output
      diagnostic classifications are recorded. Keep the
      `coproc.tests` upstream `.right` bridge enabled until that gate is
      complete.

## 2026-08-14 Coproc Child fd2 Follow-up

The bridge-free official source body was rerun after moving coproc child
stderr onto the parent shell fd2 owner. The raw comparison is stored at
`target/issue-suites/results/coproc-actual-20260814-coproc-fd2/`.

The previous `compound_exec.rs` setup used `Stdio::inherit()` for every
coproc child's stderr. That inherited the Rubash process stderr handle and
ignored `exec 2>&1`. The implementation now selects a forwarding target from
the parent `FdTable`, pipes child stderr, forwards chunks to stdout/stderr,
files, a closed sink, or an existing coproc writer, and joins the forwarding
thread when the job is reaped. Explicit `2>`, `2>>`, and `2>&-` coproc
redirects remain applied after the default fd2 setup.

Verification:

- [x] `cargo test --test cli_tests c_command_coproc_child_inherits_persistent_stderr_to_stdout -- --nocapture` -> 1/1.
- [x] `cargo test --test cli_tests coproc -- --nocapture` -> 16/16.
- [x] `cargo test --test executor_tests command_chaining::part_080 -- --nocapture` -> 152/152.
- [x] Bridge-free Bash and Rubash source bodies both exit `0`.
- [x] `xcase: command not found` is in Rubash stdout and Rubash stderr is
      empty for the bridge-free body.

Remaining differences are separate owners:

- [ ] Replace the coproc array's `(pid pid)` virtual values with stable shell
      fd values or document a deliberate Windows representation. Bash prints
      `63 60`; Rubash currently prints the child PID twice.
- [ ] Match Bash's `echo: write error: Bad file descriptor` after
      `exec >&${COPROC[1]}-`; Rubash still emits an empty line and reports
      only the later `4: Bad file descriptor` from `read <&4`.

## 2026-08-14 Pipeline Stage fd2 Follow-up

The bridge-free source body was rerun with the same GNU Bash source copied to
`coproc-actual-body.sh`, so the Rubash upstream handler did not match the file
name. The raw comparison is stored at
`target/issue-suites/results/coproc-actual-20260814-pipeline-fd2/`.

The real pipeline difference was in the Windows concurrent external-pipeline
owner. It configured stderr only for the last stage and wrote that output to
host stderr. Intermediate stages therefore bypassed `exec 2>&1`. The owner in
`src/executor/pipeline_exec.rs` now captures every stage's stderr when the
shell fd2 endpoint is `FdWriteEndpoint::Stdout`; intermediate readers run on
threads so a large diagnostic cannot fill a pipe and deadlock the pipeline.
The concurrent path is also disabled while stdout capture is active, because
inheriting a native handle would bypass shell capture.

Verification:

- [x] `cargo test --test cli_tests c_command_pipeline_stages_inherit_persistent_stderr_to_stdout -- --nocapture` -> 1/1.
- [x] `cargo test --test executor_tests command_chaining::part_080 -- --nocapture` -> 152/152.
- [x] `cargo test --test cli_tests coproc -- --nocapture` -> 15/15.
- [x] Both bridge-free source-body processes exit `0`.
- [x] The Rubash pipeline `/etc/passwd` diagnostic is now in bridge-free
      stdout, not bridge-free stderr.

Remaining differences in this artifact are intentionally separate TODOs:

- [ ] `xcase` is unavailable on the Windows host; decide on a fixture or
      host-owned classification.
- [ ] Coproc child stderr still uses inherited process stderr and does not
      follow the parent shell's virtual fd2 endpoint.
- [ ] Coproc endpoint display and closed-output diagnostics still differ from
      Bash as listed above.

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
| `coproc` | `0/0` | semantic kernel covered for endpoint lifetime, virtual fd display, external children, pipeline-stage fd2, coproc-child fd2, and closed-output diagnostics; official output remains open only for host fixtures and ledger refresh | [x] Add and pass loop, external `cat`/`cat -`, duplicate/move fd, completed cleanup, cross-process termination, direct/intermediate/coproc-child fd2, virtual-fd display, and closed-output diagnostic regressions. [ ] Classify missing `xcase` and `/etc/passwd` as host fixtures, then refresh the official actual-output row without the `.right` bridge. |
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
| `parser` | `2/0` | partial semantic fix: malformed parameter expansion now returns rc=2; remaining grammar differences are concentrated in parser fixture edge cases | [x] Add unmatched `${...` rc=2 regression. [ ] Enumerate remaining malformed compound-command cases and preserve valid newline forms. |
| `posix2` | `9/0` | isolate first: termination/status mismatch | [ ] Reproduce with process tracing and timeout cleanup; determine whether the status comes from a host process, test harness, or Rubash. |
| `posixexp` | `2/0` | partial semantic fix: unterminated `${...` now reports syntax status 2; valid POSIX expansion and RHS/IFS behavior remain open | [x] Cover `${x`, `${x/foo`, and `${x:?` rc=2. [ ] Split remaining invalid syntax from valid POSIX expansion. |
| `posixpipe` | `0/0` | semantic lead: pipeline stdin/fd lifecycle and POSIX status | [ ] Compare pipeline member status, `PIPESTATUS`, `pipefail`, and stdin closure in a minimal fixture. |
| `printf` | `2/0` | semantic lead: builtin format/argument parsing and status | [ ] Separate invalid format, missing argument, numeric conversion, and escape behavior; add builtin tests. |
| `procsub` | `124/0` | semantic lead plus host boundary: process-substitution fd and path lifecycle | [ ] Run each `<(...)`/`>(...)` primitive with timeout; verify endpoint ownership, child completion, path translation, and cleanup. |
| `quote` | `127/0` | isolate first: quote body likely affected by command lookup/environment | [ ] Rerun self-contained quoting cases; only then assign expansion fixes. |
| `quotearray` | `0/0` | semantic lead: quoted indexed/associative array expansion | [ ] Compare `${array[@]}`, `${array[*]}`, empty members, and nested quotes; add expansion regressions. |
| `read` | `124/0` | semantic lead plus host boundary: redirected stdin/fd EOF and read loop | [ ] Run each `read` case with timeout; inspect fd ownership, heredoc/input precedence, close behavior, and EOF. |
| `redir` | `124/0` | semantic lead plus host boundary: ordered redirection and fd lifetime | [ ] Split ordered dup/close, `/dev/null`, dynamic fd, heredoc, and external-child cases; fix the shared fd owner and add regressions. |
| `rhs-exp` | `0/0` | semantic lead: parameter substitution RHS expansion and quote rules | [ ] Extract pattern replacement, backslash, empty RHS, and command-substitution cases; add parameter-expansion tests. |
| `rsh` | `0/0` | host/path boundary candidate: restricted-shell invocation and command lookup | [ ] Reproduce with identical PATH and working directory; separate restricted-shell semantics from Windows command availability. |
| `set-e` | `0/0` | partial semantic fix: pipeline status propagation now honors `pipefail` under `errexit`; conditional, command-substitution, function, and subshell suppression still require coverage | [x] Add a regression for `set -e -o pipefail; false | true`. [ ] Compare remaining suppression rules and ERR-trap interactions. |
| `set-x` | `0/0` | semantic lead: tracing output and command expansion state | [ ] Compare PS4, quoting, redirections, functions, and command substitutions; define whether stderr trace output is in the contract. |
| `shopt` | `0/0` | native probes pass; remaining `set +o igncr` listing is Windows CRLF/input-policy output | [ ] Keep the upstream bridge until shopt option mutation/query parity is covered; record `igncr` as host-owned unless a native CRLF parser-input contract is added. |
| `test` | `0/0` | semantic lead: test expression parsing, operators, and status | [ ] Extract string, numeric, file, and compound expression differences; add builtin tests without hardcoding output. |
| `tilde2` | `0/0` | semantic lead: tilde expansion in assignments, paths, and quoted contexts | [ ] Compare home, login, assignment, and path-shaped inputs under the Windows home mapping. |
| `trap` | `2/0` | semantic lead: trap option parsing, invalid signals, and status/diagnostic propagation | [ ] Split `-l`, `-p`, `-P`, action-only, invalid signal, and execution timing cases; add trap regressions. |
| `type` | `127/0` | isolate first: command lookup/PATH mismatch likely | [ ] Reproduce with a controlled PATH and command fixture; then align builtin, function, alias, and file classification. |
| `varenv` | `0/0` | semantic lead: exported environment, shell state, and child inheritance | [ ] Compare scalar/array export, special variables, subshell state, and child environment serialization. |
| `vredir` | `0/0` | partial semantic fix: lowest-free dynamic fd reuse and nameref/array storage now match; varredir failure recovery, diagnostics, and presentation remain open | [x] Re-run `vredir4`, `vredir5`, and `vredir7` without the bridge; add FdTable and CLI regressions for reuse and nameref targets. [ ] Fix `vredir8` failed `<>` allocation state and classify `/dev/tty`; decide whether function pretty-print and variable-name diagnostics are contract or formatting TODOs. |

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

## 2026-08-14 Current Coproc Artifact

The newest bridge-free comparison is stored at
`target/issue-suites/results/coproc-actual-20260814-closed-output/`. The source
was copied to `coproc-actual-body.sh`, so it does not match the
`coproc.tests` filename handler. It was run against the working tree at
`6d83e106c9dfe814c180203fb3a3f8be68ea22ce` with GNU Bash
`D:/Git/bin/bash.exe`; both shells completed with status `0`.

| Difference | Status | Durable interpretation / TODO |
|---|---|---|
| `COPROC[0]`/`COPROC[1]` values | Done | Rubash exposes stable shell-owned virtual descriptors (`10 11`, then `12 13`, then `14 15`). Bash's native values (`63 60`) are implementation-specific and are not a required Windows representation. The distinct-fd regression is `c_command_exposes_distinct_virtual_fds_for_named_coproc`. |
| `echo ${COPROC[@]}` after `exec >&${COPROC[1]}-` | Done | Shared `write_ordered_command_output` now detects a persistent closed fd even when the current command has no redirect and emits `rubash: echo: write error: Bad file descriptor`. Regression: `c_command_echo_reports_persistent_closed_stdout`. |
| `xcase` command | Open, host fixture | `xcase` is absent on this Windows host. Keep this as a harness/host-owned fixture decision; do not add a coproc-specific output shortcut. |
| `/etc/passwd` pipeline | Open, host fixture | `/etc/passwd` is absent on Windows. The pipeline fd2 routing is covered; provide a project fixture or classify the Unix path as outside the Windows contract. |
| Diagnostic spelling/locale | Open, host fixture | Bash prints `cat: ... No such file or directory`; WinuxCmd prints the equivalent localized `cat： ... 没有那个文件或目录`. Decide whether actual-output comparisons normalize host utility locale. |

The focused verification for this slice is:

- [x] `cargo test --test cli_tests c_command_echo_reports_persistent_closed_stdout -- --nocapture` -> 1/1.
- [x] `cargo test --test cli_tests c_command_coproc_child_inherits_persistent_stderr_to_stdout -- --nocapture` -> 1/1.
- [x] `cargo test --test cli_tests coproc -- --nocapture` -> 17/17.
- [x] `cargo test --test executor_tests command_chaining::part_080 -- --nocapture` -> 152/152.
- [ ] Refresh the official `coproc` `.tests` ledger row after the host fixture and locale decisions are recorded. The `.right` bridge remains enabled until this gate is complete.

## 2026-08-14 Vredir FD Reuse And Nameref Artifact

The focused `vredir` difference ledger is maintained separately in
[`docs/vredir-diff-todo.md`](vredir-diff-todo.md). It is the source of truth
for the current `vredir8` Bash/Rubash outputs, ownership, and remaining TODOs.

### 2026-08-14 varredir regression refresh

The newer bridge-free artifact is
`target/issue-suites/results/native-bash-20260814-vredir-varredir-regression/`.
`vredir4.sub`, `vredir5.sub`, `vredir7.sub`, and `vredir8.sub` all return `0`
under both GNU Bash and Rubash. `vredir8` stdout is identical; its remaining
stderr differences are the Windows `/dev/tty` host diagnostic and Bash's
source-token `$fd` versus Rubash's expanded numeric fd in the closed-fd
diagnostic. `vredir4/5/7` retain only function pretty-print formatting
differences. The detailed TODOs and ownership decisions are maintained in
[`docs/vredir-diff-todo.md`](vredir-diff-todo.md).

The bridge-free native Bash comparison is stored at
`target/issue-suites/results/native-bash-20260814-vredir-fd-reuse-nameref-verified/`.
It was generated from the current working tree after rebuilding Rubash; the
repository HEAD used as the base is `6d83e10`. Each body has Bash and Rubash
stdout, stderr, and status files.

| Body | Result | Interpretation and TODO |
|---|---|---|
| `vredir4.sub` | Bash/Rubash `0/0` | Dynamic descriptors now reuse `10` and `11`; nameref assignment writes the target variables and both expansions show `10 11`. Remaining stdout differs only in function pretty-print spacing/semicolons. The final error is the same `Bad file descriptor` class, but Bash names `${output}` while Rubash prints the expanded `11`. [ ] Decide whether the pretty-printer and source-token diagnostic spelling are compatibility contract; if so, fix at the command-text/diagnostic owner. |
| `vredir5.sub` | Bash/Rubash `0/0` | Ordered input/output moves and heredoc precedence now match, including `12 10`. Remaining difference is only function pretty-print formatting. [ ] Add a formatting classification or align the function renderer; do not change FD semantics for this row. |
| `vredir7.sub` | Bash/Rubash `0/0` | Indexed dynamic names `{fd[0]}` and `{fd[1]}` now match Bash, including `12 10`, array close, and input/output move behavior. Remaining difference is only function pretty-print formatting. [ ] Classify or align renderer output. |
| `vredir8.sub` | Bash/Rubash `0/1` | `/dev/tty` is unavailable in this non-interactive Windows host, so that open is host-owned. Independently, Rubash reports `ambiguous redirect` after the failed dynamic `<>` allocation and omits Bash's `redir 2`; this is a real failed-varredir state/status TODO. [ ] Preserve Bash's closed/value state after failed `<>`, make the subsequent `>&$fd` diagnostics follow the closed-fd path, and keep host `/dev/tty` behavior separate. |

Focused evidence:

- [x] `cargo test --lib fd_table -- --nocapture` -> 3/3.
- [x] `cargo test --test cli_tests c_command_reuses_closed_dynamic_fds_and_resolves_nameref_targets -- --nocapture` -> 1/1.
- [x] `cargo check` passes; existing warnings are dead-code/unused-import warnings.
- [x] The artifact is bridge-free and preserves stdout, stderr, and status for
      all four bodies.
- [x] Bounded `run-redir` -> 1/1 and `run-vredir` -> 1/1 after the FD reuse and
      nameref changes; logs are under
      `target/bash-upstream-tests/logs/run-redir.log` and
      `target/bash-upstream-tests/logs/run-vredir.log`.
- [ ] Refresh the official ledger row after the `vredir8` failed-varredir
      state is fixed or explicitly classified as host-owned. The upstream
      `.right` bridge stays enabled until that gate is satisfied.
