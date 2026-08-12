# GNU Bash Compatibility Implementation Plan

> Date: 2026-08-12
> Audience: future agents and maintainers fixing rubash compatibility.
> Goal: make rubash reach GNU Bash observable compatibility while staying usable
> as the Bash syntax/execution library embedded by Winuxsh.

## Current Architecture Reality

Older docs map GNU Bash C files to Rust owner modules. That map is still useful
for provenance, but it is **not** the architecture to implement literally.
Rubash is no longer just a `bash.exe`-shaped binary. It is becoming the Bash
grammar and execution library used by Winuxsh.

The correct implementation model is:

```text
GNU Bash observable behavior
        |
        v
rubash semantic modules
  lexer/parser/AST/expansion/redirection/builtins/jobs/runtime
        |
        v
winuxcmd backend primitives
  Windows process, handle, fd, pipe, device, hard-kill operations
        |
        v
winuxsh host integration
  interactive shell, rc/profile/session wrapper, embedding rubash as a library
```

Do not organize new work around a file-for-file GNU Bash port. Organize around
semantic ownership and make each module traceable back to GNU Bash sources and
test suites.

## Architecture Rules

| Concern | Owner | Notes |
|---|---|---|
| Bash grammar, AST, parse errors | `src/lexer/`, `src/parser/` | Must match Bash rc=2/error behavior for invalid syntax. |
| Word/parameter/command/arithmetic expansion | `src/expand/`, `src/executor/expand_*`, `src/executor/arithmetic/` | Expansion behavior is a Bash semantic surface, not host-shell behavior. |
| Redirection syntax and fd semantics | `src/executor/redirection.rs`, `src/executor/*redirect*`, future fd abstraction | `2>&1`, `&1`, close, dup, fd-word expansion, diagnostics. |
| Windows fd/device/process primitives | `src/sys/` plus winuxcmd integration boundary | `/dev/null`, `NUL`, handles, pipes, hard kill, process existence. |
| Builtin commands | `src/builtins/` plus executor adapters | Match Bash option parsing, output, diagnostics, and statuses. |
| Job/trap/signal semantics | `src/jobs/`, `src/builtins/{kill,trap,jobs,wait,fg_bg}.rs` | Bash job-control model; Windows process ops delegated to backend. |
| Readline editing | `src/input/readline/` | Interactive editing only. It is not the same as shell builtins. |
| Winuxsh shell host | external `winuxsh` repo | Should embed rubash and provide shell UX; should not own Bash grammar fixes. |

### Duplicate Names Are Expected

Some Rust files share names because they map to different GNU Bash subsystems.
Do not merge them just because the filename matches.

| File | Meaning | GNU Bash owner |
|---|---|---|
| `src/builtins/kill.rs` | Bash `kill` builtin: signal parsing, job/pid operands, process dispatch | `builtins/kill.def` |
| `src/input/readline/kill.rs` | Readline kill/yank editing commands | `lib/readline/kill.c` |

The same rule applies to future collisions: module path and semantic owner
matter more than basename.

## Current Compatibility Debt

The latest suite analysis is in
[`docs/issue-suite-diff-analysis.md`](issue-suite-diff-analysis.md). The short
version:

1. Redirection/fd/device handling is incomplete.
2. Heredoc collection/runtime is incomplete and can hang on large input.
3. Parser strictness is too permissive in many Bash rc=2 cases.
4. Arithmetic errors do not consistently propagate.
5. Alias/hash behavior is partial.
6. Word expansion still has many edge-case failures.
7. Jobs/coproc/process substitution need a coherent fd/process model.
8. Builtins exist but option/error parity is uneven.

`src/executor/upstream_scripts*` is compatibility scaffolding. Do not remove it
while the suite corpus is red. Replace it only after real semantics and tests
cover the behavior it masks.

## Definition of Done

Rubash is considered GNU Bash-compatible only when all of these hold:

| Gate | Requirement |
|---|---|
| Rust tests | `cargo test --lib`, integration tests, and focused regression tests pass. |
| Self difftest | `tests/difftest/` passes against GNU Bash for all tracked cases. |
| Bash upstream `.right` | project upstream runner passes all `run-*` groups without unclassified log noise. |
| Bash actual-output | official `.tests` bodies match current GNU Bash actual output or every difference is documented as environment-only. |
| Oil spec | Bash/POSIX-relevant spec diffs converge to zero or documented non-targets. |
| mksh | Bash-compatible subset diffs converge to zero; ksh-only syntax is excluded. |
| busybox ash | Bash/POSIX-relevant redir/heredoc/vars/signals/parser diffs converge to zero. |
| ksh93 | Bash-compatible subset diffs converge to zero; ksh-only syntax is excluded. |
| No scaffolding reliance | `upstream_scripts` output spoofing is unused or removed after real behavior covers it. |

Do not claim progress from one passing suite if another suite shows the same
root-cause family still failing.

## Agent Workflow

Every compatibility agent should follow this loop:

1. **Read context**
   - `docs/issue-suite-diff-analysis.md`
   - this document
   - `docs/bash-compat-issues.md`
   - `docs/bash-source-map.md`
   - the relevant GNU Bash source/test file.
2. **Pick one root-cause family**
   - Do not mix fd, heredoc, arithmetic, alias, and builtin work in one patch
     unless a single underlying abstraction requires it.
3. **Find the smallest failing reproducer**
   - Prefer an existing Rust test or add one.
   - If the failure comes from a large suite, extract the minimum script first.
4. **Fix the semantic owner**
   - Fix parser in parser modules, fd behavior in fd/redirection modules,
     builtin behavior in builtins, backend operations in sys/winuxcmd boundary.
   - Do not patch expected-output dispatch as the primary fix.
5. **Run focused tests**
   - Run the new Rust regression.
   - Run the smallest suite slice that originally failed.
6. **Run broader gates**
   - At minimum run Rust tests plus the affected suite family.
   - For fd/heredoc/jobs, run busybox + bash actual-output slices because these
     expose hangs and descriptor bugs.
7. **Update docs/issues**
   - Update issue comments with exact suite result, artifact path, and root
     cause.
   - Update docs if the root-cause classification or architecture boundary
     changes.
8. **Commit**
   - Commit code and tests together.
   - Keep generated logs under `target/` untracked.

## Test Execution Playbook

### Always Avoid Unbounded Full Runs

Full suites can hang. Use per-test or per-file timeouts whenever possible.
If a suite has no native timeout, wrap each file or directory separately.

Do not run large upstream scripts directly from user directories. Use isolated
work dirs under `target/`.

### Baseline Commands

Use these from `D:/repo/rubash` unless a command says otherwise.

```sh
cargo test --lib
cargo test --test cli_tests c_command_kill_ -- --nocapture
cargo test --test executor_tests test_kill -- --nocapture
```

Bash upstream `.right` runner:

```sh
BASH_RUNNER=D:/Git/bin/bash.exe D:/Git/bin/bash.exe scripts/run-bash-upstream-tests.sh
```

Focused Bash upstream runner:

```sh
BASH_RUNNER=D:/Git/bin/bash.exe D:/Git/bin/bash.exe scripts/run-bash-upstream-tests.sh run-redir
```

Bash actual-output runner:

```sh
D:/Git/bin/bash.exe target/issue-suites/run-bash-actual-difftest.sh
```

mksh official runner with timeout:

```sh
cd target/issue-suites/mksh
D:/Git/usr/bin/perl.exe check.pl -v -t 10 -p ./rubash-under-test.exe -s check.t
```

Busybox official runner:

```sh
cp D:/repo/rubash/target/debug/rubash.exe target/issue-suites/busybox/shell/ash_test/ash
touch target/issue-suites/busybox/shell/ash_test/.config
cd target/issue-suites/busybox/shell/ash_test
D:/Git/bin/bash.exe ./run-all > D:/repo/rubash/target/issue-suites/results/busybox-run-all.log 2>&1
```

ksh93 diff wrapper:

```sh
D:/Git/bin/bash.exe target/issue-suites/run-ksh93-difftest.sh
```

Oil spec wrapper:

- Use the existing local copy under `target/issue-suites/results/oils-py3`.
- Keep Python3 harness fixes in the copied result tree, not the upstream
  checkout.
- Run per spec file with timeout; do not run the whole suite unbounded.

### Result Handling

- Raw results go under `target/issue-suites/results/`.
- Durable interpretation goes in `docs/`.
- Issue comments should include:
  - command or runner;
  - total/pass/fail/diff counts;
  - high-signal failing files;
  - root-cause family;
  - artifact paths.

### Process Cleanup

On Windows, some Git Bash or child processes can survive a timed-out run.
Check:

```sh
ps -ef | rg 'rubash.exe|bash.exe|run-.*difftest|shtests|check.pl'
```

Prefer targeted cleanup:

```sh
taskkill.exe /PID <pid> /T /F
```

Do not use broad destructive cleanup against the repo or user directories.

## Implementation Roadmap

### Phase 1: fd/device/redirection foundation

Why first: `/dev/null`, `2>&1`, `&1`, ambiguous redirects, coproc descriptors,
and read/mapfile fd tests all depend on the same model.

Implement:

- A central fd table abstraction for shell-visible fds.
- Device normalization for `/dev/null`, `NUL`, and Bash-style null redirection.
- fd word expansion before opening/duplicating descriptors.
- Bash-compatible dup/close semantics:
  - `n>&m`
  - `n<&m`
  - `n>&-`
  - `n<&-`
  - invalid fd diagnostics;
  - ambiguous redirect diagnostics.
- Redirection application order exactly left-to-right.
- Persistent fd changes for `exec`, temporary redirs for simple commands and
  builtins.

Primary modules:

- `src/executor/redirection.rs`
- `src/executor/external_redirects.rs`
- `src/executor/builtin_redirects.rs`
- `src/executor/read_redirected_fd.rs`
- `src/sys/sh/zmapfd.rs`
- future shared fd table module if needed.

Tests to add first:

- `echo ok >/dev/null`
- `echo err 2>/dev/null`
- `echo both >/dev/null 2>&1`
- `echo hi 3>&1 1>/dev/null >&3`
- `exec 3>&1; echo hi >&3; exec 3>&-; echo fail >&3`
- `echo hi >&bad` and `echo hi > "$multi word"` ambiguous cases.

Suite slices:

- Bash actual-output: `redir`, `vredir`, `read`, `coproc`
- Busybox: `ash-redir`, `ash-signals` after kill/fd interaction changes

### Phase 2: heredoc correctness and performance

Implement:

- Lexer collection for `<<`, `<<-`, quoted delimiters, and multiple heredocs.
- Correct interaction with command substitution and subshell parsing.
- Unterminated heredoc warnings and statuses.
- Streaming delivery for large heredocs so `heredoc_huge` cannot hang or blow
  memory.

Primary modules:

- `src/lexer/heredoc.rs`
- `src/lexer/heredoc_scan.rs`
- `src/executor/parse_helpers.rs`
- `src/executor/read_redirected_fd.rs`
- `src/executor/command_substitution*.rs`

Tests to add first:

- single heredoc;
- `<<-` tab stripping;
- quoted delimiter no expansion;
- multiple heredocs on one command line;
- heredoc inside `$(...)`;
- missing delimiter warning;
- large heredoc through external command.

Suite slices:

- Bash actual-output: `heredoc`, `comsub-eof`, `comsub-posix`, `exportfunc`
- Busybox: `ash-heredoc`, especially `heredoc_huge`
- mksh: heredoc groups

### Phase 3: parser strictness

Implement:

- Bash-compatible rc=2 for invalid conditional syntax.
- Invalid array assignment syntax rejection.
- Invalid extglob / glob bracket errors.
- Arithmetic-for syntax errors.
- Correct line-numbered diagnostics where suites compare stderr.

Primary modules:

- `src/parser/grammar.rs`
- `src/parser/conditional_command.rs`
- `src/parser/array_element_assignment.rs`
- `src/parser/arithmetic_for.rs`
- `src/parser/extglob_pattern.rs`
- `src/lexer/*`

Suite slices:

- Bash actual-output: `parser`, `cond`, `errors`, `glob-bracket`,
  `arith-for`, `array`
- Oil: `parse-errors`, `shell-grammar`, extglob bad syntax
- Busybox: `ash-parsing`

### Phase 4: arithmetic errors

Implement:

- invalid base/value detection;
- division-by-zero propagation;
- invalid lvalue assignment diagnostics;
- arithmetic syntax errors inside `$(( ))`, `(( ))`, `let`, conditions, and
  arithmetic-for.

Primary modules:

- `src/executor/arithmetic/*`
- `src/expand/arithmetic.rs`
- `src/builtins/let.rs`

Suite slices:

- Bash actual-output: `arith`, `arith-for`, `cond`, `quotearray`
- Oil: `arith`
- mksh: arithmetic / integer-base Bash-compatible subset

### Phase 5: jobs, kill, coproc, process substitution

Implement:

- Keep `src/builtins/kill.rs` as the Bash builtin owner.
- Keep `src/input/readline/kill.rs` as readline editing owner.
- Model Bash signal names/statuses in `jobs/signals.rs` or shared signal table.
- Use winuxcmd/backend primitives for Windows process termination and existence
  checks.
- Preserve SIGKILL hard-kill behavior: it must bypass rubash trap/mailbox
  delivery and terminate the target process.
- Define coproc fd ownership and cleanup.
- Integrate `wait`, `jobs`, `fg/bg`, `disown`, process substitution, and
  background jobs through one job table.

Primary modules:

- `src/builtins/kill.rs`
- `src/builtins/wait.rs`
- `src/builtins/jobs.rs`
- `src/builtins/fg_bg.rs`
- `src/jobs/*`
- `src/parser/coproc_command.rs`
- `src/parser/process_substitution.rs`
- executor pipeline/background modules.

Suite slices:

- Rust kill regressions
- Bash actual-output: `jobs`, `coproc`, `procsub`
- Busybox: `ash-signals`
- Oil: `background`, `builtin-kill`, `builtin-process`

### Phase 6: word expansion and variables

Implement:

- IFS empty-field behavior.
- `$@`, `$*`, quoted arrays, associative arrays.
- substring/slice negative offsets and lengths.
- pattern substitution and quote removal.
- command substitution state isolation.
- variable attributes that are Bash-relevant (`declare`, `local`, `readonly`,
  `export`, `nameref`, integer).

Primary modules:

- `src/executor/expand_word.rs`
- `src/executor/parameter_words.rs`
- `src/executor/expand_braced_*`
- `src/shell/arrays/*`
- `src/builtins/declare*`
- `src/executor/command_substitution*.rs`

Suite slices:

- Bash actual-output: `ifs`, `nquote*`, `quotearray`, `rhs-exp`, `assoc`,
  `array`, `nameref`
- Oil: `word-split`, `var-op-*`, `array*`, `nameref`
- ksh93: `substring`, `quoting`, Bash-compatible variable cases

### Phase 7: builtin parity

Implement builtin-by-builtin parity. Each builtin needs:

- option parser parity;
- output format parity;
- stderr diagnostic parity;
- exit status parity;
- redirection interaction parity;
- POSIX mode differences where applicable.

High-yield order:

1. `read` / `mapfile`
2. `trap`
3. `umask`
4. `set` / `shopt`
5. `type` / `command` / `hash`
6. `printf`
7. `complete` / `compgen` / `compopt`
8. `getopts`
9. `cd` / `pushd` / `dirs`

Suite slices:

- Bash actual-output by builtin file.
- Oil `builtin-*`.
- Busybox ash builtin directories where POSIX-relevant.

### Phase 8: remove compatibility scaffolding

Only after suite slices pass through real implementation:

1. Mark which `upstream_scripts` handler is covered by real tests.
2. Disable that handler behind a temporary flag.
3. Run affected suite slice.
4. Remove handler and expected-output include.
5. Commit with tests and suite result.

Never remove the whole scaffold in one patch while suites are still red.

## Documentation Update Requirements

When code moves or architecture changes:

- Update `docs/source-layout.md` if ownership boundaries change.
- Update `docs/bash-source-map.md` if GNU Bash source ownership moves.
- Update `docs/bash-implementation-inventory.md` if a target module is renamed.
- Update `docs/issue-suite-diff-analysis.md` when suite counts or root-cause
  classification changes.
- Mention whether the change affects rubash only, winuxcmd backend behavior, or
  Winuxsh integration.

## Agent Checklist

Before editing:

- [ ] Identify root-cause family.
- [ ] Identify semantic owner module.
- [ ] Read related GNU Bash test/source.
- [ ] Create or locate smallest reproducer.

During editing:

- [ ] Add focused Rust test or difftest case.
- [ ] Fix real semantics, not output spoofing.
- [ ] Avoid broad refactors outside the root-cause family.
- [ ] Preserve unrelated user changes.

Before final response:

- [ ] Run focused tests.
- [ ] Run at least one affected suite slice.
- [ ] Check no stuck `rubash.exe` / `bash.exe` / suite process remains.
- [ ] Update issue/doc if result changes.
- [ ] Commit if the user asked for durable work or the session is tracking
      progress through commits.
