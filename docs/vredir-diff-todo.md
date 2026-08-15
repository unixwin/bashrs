# `vredir` Actual-Output Difference TODO

> Snapshot: 2026-08-14
> Scope: GNU Bash `vredir*.sub`, with the current focus on `vredir8`.
> Initial raw evidence: `target/issue-suites/results/vredir8-probe-20260814/`
> Latest bridge-free evidence: `target/issue-suites/results/native-bash-20260814-vredir-varredir-regression/`

This document owns the durable interpretation of the current `vredir` probe.
The raw Bash and Rubash stdout, stderr, and status files remain under
`target/issue-suites/results/`; this file records why a difference exists and
what must happen before it can be closed. A passing `.right` runner is not
evidence that this TODO is closed.

## Current Owners

| Semantic area | Rust owner | Status |
|---|---|---|
| Dynamic fd allocation, lowest-free reuse, and capability separation | `src/executor/fd_table.rs` | real, focused tests pass |
| Dynamic varredir open, duplicate, move, close, and nameref resolution | `src/executor/trap_exec.rs` | real, focused tests pass |
| Redirect application and persistent output diagnostics | `src/executor/redirection.rs` | real, focused tests pass |
| `read` from numbered and dynamic input fds | `src/executor/read_builtin.rs`, `src/executor/read_io.rs` | real, focused tests pass |
| External-child fd materialization | `src/executor/external_setup.rs`, `src/executor/fd_table.rs` | partial; next fd gate |
| Windows device/path and localized OS diagnostics | host/WinuxCmd integration | host-owned candidate |

## Observed Differences

### `vredir8` valid dynamic read/write fd

The Bash and Rubash probe outputs agree for `: {fd}<>/dev/null`: the dynamic
variable receives fd `10`, output through `>&$fd` succeeds, and the explicit
close succeeds. This is closed as a semantic difference.

Evidence: `valid-null.bash.*` and `valid-null.rubash.*` in the raw artifact
directory.

### Failed open of a missing path

The Bash and Rubash probe outputs agree for a missing path: the dynamic
variable remains unset, the command status is reported as `0` by the probe
wrapper, and the later use is not treated as a newly allocated fd.

Evidence: `failed-missing.bash.*` and `failed-missing.rubash.*`.

### Failed open of `/dev/tty`

Two independent facts must not be merged:

1. `/dev/tty` is not available in this Windows-hosted environment. Bash
   reports `No such device or address`; Rubash reports the localized Windows
   path error. This is a host/device and diagnostic-locale difference.
2. The latest bridge-free `vredir8.sub` run returns status `0` in both Bash and
   Rubash and has identical stdout. The failed open does not create a dynamic
   variable, and later commands continue with the expected status.

Evidence: `failed-tty.bash.*` and `failed-tty.rubash.*`.

Status: dynamic varredir semantics closed. The `/dev/tty` path and localized
OS diagnostic remain host-owned. The later closed-fd diagnostic token is
tracked separately below.

### `varredir_close`

The Bash and Rubash probe stdout and status agree: the variable keeps its
numeric value after the command-local auto-close, a later write fails, and an
explicit close remains harmless. The only recorded stderr difference is:

```text
Bash:   $fd: Bad file descriptor
Rubash: 10: Bad file descriptor
```

This is source-token rendering, not fd ownership: the virtual fd is already
closed in both shells. It belongs to diagnostic rendering, not to
`FdTable` allocation or varredir lifetime.

Evidence: `varredir-close.bash.*` and `varredir-close.rubash.*`.

Status: fd semantics closed; diagnostic token policy open.

### `vredir4/5/7` function rendering

The latest bridge-free run shows matching fd values, move/close behavior, and
status for all three tests. Rubash prints function bodies with different
spacing/semicolons from Bash. This is a command pretty-printer difference and
does not belong to `FdTable` or redirection execution.

Evidence: `vredir4.*`, `vredir5.*`, and `vredir7.*` in the latest artifact.

Status: open output-format TODO; no fd semantic regression is indicated.

## TODO

- [x] Keep `<>` dynamic varredir state in `FdTable` and mirror the variable
      only after a successful open.
- [x] Support dynamic `>&1` and input/output capability duplication without
      granting an unrelated capability.
- [x] Reuse the lowest closed dynamic fd slot.
- [x] Preserve dynamic variable values after close and implement
      `varredir_close` auto-close.
- [x] Continue executing later commands after a dynamic varredir open failure;
      preserve the command's failure status without aborting the AST.
- [x] Add CLI regressions for valid `<>`, `>&1`, `varredir_close`, and failed
      open followed by another command.
- [x] Run the new regressions and `command_chaining::part_080` against the
      current worktree.
- [x] Re-run bridge-free `vredir4`, `vredir5`, `vredir7`, and `vredir8` with
      per-test timeout and save
      `target/issue-suites/results/native-bash-20260814-vredir-varredir-regression/`.
- [x] Re-run bounded `run-redir` and `run-vredir` after the regression pass;
      both passed 1/1.
- [x] Preserve Bash's original source token (`$fd`) in closed-fd diagnostics;
      the fix belongs to command/diagnostic rendering, not `FdTable`. The
      focused regression is `c_dynamic_fd_closed_redirect_preserves_source_token_diagnostic`.
- [ ] Define a Windows fixture or explicit host-owned policy for `/dev/tty`
      and its localized OS diagnostic.
- [ ] Decide whether function pretty-print spacing/semicolons is part of the
      actual-output contract; if so, assign it to the function renderer.
- [ ] Refresh the official actual-output row without relying on `.right`,
      recording the host-owned `/dev/tty` difference and the two output-format
      decisions above.
- [ ] After the preceding gates, update the `vredir` row in
      `docs/bash-actual-output-diff-todo.md` and
      `docs/issue-suite-diff-analysis.md` to point here as the source of truth.

## Verification Commands

Use bounded commands and preserve their raw output under
`target/issue-suites/results/`:

```text
cargo check
cargo test --test cli_tests c_command_dynamic_varredir -- --nocapture
cargo test --test executor_tests command_chaining::part_080 -- --nocapture
BASH_RUNNER=D:/Git/bin/bash.exe D:/Git/bin/bash.exe scripts/run-bash-upstream-tests.sh run-redir
BASH_RUNNER=D:/Git/bin/bash.exe D:/Git/bin/bash.exe scripts/run-bash-upstream-tests.sh run-vredir
```

Before finishing a test turn, check for stuck `rubash.exe`, `bash.exe`,
`cargo.exe`, and suite-runner processes.
