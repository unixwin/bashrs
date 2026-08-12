# Issue Suite DIFF Analysis

> Date: 2026-08-12
> Scope: issue #20-#26 compatibility suites, local reruns, and implementation ownership.

This document is the durable version of the issue-suite run notes. Files under
`target/issue-suites/results/` are raw run artifacts; this document is the
tracked summary used to decide what to fix and where.

## Executive Summary

The failures are not one-off test noise. They cluster into a small number of
Bash semantic subsystems:

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
| Arithmetic diagnostics/status | `arith`, `arith-for`, `cond`, `quotearray`, `new-exp` | `src/executor/arithmetic/*`, `src/expand/arithmetic.rs`, `src/builtins/let.rs` |
| Heredoc / command substitution | `heredoc`, `comsub-eof`, `comsub-posix`, `exportfunc` | `src/lexer/heredoc*.rs`, `src/parser/command_substitution.rs`, `src/executor/command_substitution*.rs` |
| Redirection / fd semantics | `redir`, `vredir`, `coproc`, `procsub`, `read` | `src/executor/redirection.rs`, `src/executor/external_redirects.rs`, `src/executor/builtin_redirects.rs`, `src/sys/sh/zmapfd.rs` |
| Alias/hash semantics | `alias`, `errors`, `history` | `src/shell/alias.rs`, `src/builtins/alias.rs`, `src/builtins/hash.rs`, `src/executor/alias_*.rs` |
| Word splitting / quoting / arrays | `ifs`, `nquote*`, `quotearray`, `rhs-exp`, `assoc`, `array` | `src/executor/expand_word.rs`, `src/executor/parameter_words.rs`, `src/shell/arrays/*` |
| Glob/extglob/brace | `glob`, `globstar`, `extglob`, `braces` | `src/expand/glob/*`, `src/parser/extglob_pattern.rs`, `src/expand/braces.rs` |
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

