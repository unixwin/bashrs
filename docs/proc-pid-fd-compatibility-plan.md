# Minimal /proc/<pid>/fd Compatibility Plan

> Status: design-only blocker (2026-08-22)
> Scope: the smallest Windows/Winuxsh contract needed by real BusyBox-style programs. This is not a Linux procfs implementation.

## Finding

Rubash has a shell-owned virtual descriptor model in `src/executor/fd_table.rs`, including read/write capabilities, dynamic allocation, close state, and child materialization. It does not currently publish a filesystem namespace for those descriptors. The Rubash source tree only has special handling for `/dev/stdin`, `/dev/fd/0`, and `/proc/self/fd/0` in selected builtin paths; that is not general `/proc/<pid>/fd` compatibility and must not be treated as such.

No WinuxCmd source is present under `D:/repo`. The backend that can expose native child handles and answer `readlink/stat/readdir` for a virtual path is therefore unavailable in this workspace. Do not implement a fake directory or return guessed target strings from Rubash.

## Minimum Contract

Support only a live shell/child process descriptor directory:

- `/proc/self/fd` resolves to the calling process's compatibility view.
- `/proc/<pid>/fd` resolves only for a process registered by Winuxsh and still alive; unknown or exited pids return the normal missing-path error.
- Directory enumeration returns only currently open descriptors (at minimum 0, 1, and 2; dynamic descriptors are included while open).
- Each entry is a symlink-like object. `readlink` returns a stable logical target when one exists (regular file path, `/dev/null`, pipe endpoint), otherwise a documented opaque native-handle marker.
- Opening the path is not required initially. The supported BusyBox use case is inspection via `test -e`, `readlink`, and directory enumeration.
- Closed descriptors disappear from enumeration and fail `readlink`.
- The view is per process and snapshot-consistent for one directory operation; it must not expose another shell's descriptor table.
- No `proc`, `sys`, `mem`, `maps`, status files, PID-wide process metadata, Linux ioctl behavior, or arbitrary write support is in scope.

The path must be a provider-backed virtual namespace, not a real `proc` folder on the Windows filesystem. Keep the logical path visible to shell programs while passing native paths to unrelated Windows executables.

## Ownership

- Rubash owns Bash parsing, expansion, redirection order, virtual fd lifetime, and a child-fd snapshot/export contract.
- Winuxsh owns registration of the current shell pid and routing the virtual namespace to the selected process backend.
- WinuxCmd must own native process/handle lookup and the external commands' `readdir/stat/readlink` behavior. It must not infer Rubash shell state from environment variables.

Required cross-repo API shape (names illustrative):

1. Rubash exports `FdTable::snapshot_for_child(pid)` containing descriptor number, read/write direction, logical target, and lifetime generation.
2. Winuxsh registers that snapshot with the host/backend before launching an external command and unregisters it after process exit.
3. WinuxCmd resolves `/proc/<pid>/fd/N` through that registry, validating pid, descriptor openness, and generation before returning a link target.

The API must be capability-based and read-only. It must not expose mutable `FdEntry`s or let WinuxCmd mutate shell state.

## BusyBox-First Evidence

Use the real BusyBox ash tests as the acceptance source before adding broader shell tests. Pin the BusyBox revision and archive raw stdout/stderr under `rubash/target/issue-suites/results/proc-pid-fd/<run-id>/`. Start with individual tests, not the full suite:

```sh
# from a BusyBox ash_test checkout, after building the real ash fixture
./ash -c 'printf x >&3' 3>/tmp/proc-fd-target
./ash -c 'readlink /proc/self/fd/0; readlink /proc/self/fd/1; readlink /proc/self/fd/2'
./ash -c 'test -e /proc/self/fd/0; printf "status:%s\n" "$?"'
./ash -c 'exec 3>/tmp/proc-fd-target; readlink /proc/self/fd/3; exec 3>&-; test -e /proc/self/fd/3; printf "status:%s\n" "$?"'
```

Then run the exact BusyBox cases containing `/proc/self/fd`, `/proc/$pid/fd`, `readlink`, or fd enumeration and compare BusyBox, GNU Bash, and Rubash. Record command, BusyBox revision, host paths, exit status, timeout, stdout, stderr, and whether the command was builtin or an external WinuxCmd process. A passing Rubash-only probe is not evidence if the external `readlink` or `test` path bypasses the backend.

Expected first acceptance set:

- self fd 0/1/2 existence and readlink;
- one dynamically allocated fd, then close and disappearance;
- inherited external-child view for a regular file and a pipe;
- `/proc/<pid>/fd` rejects an exited/unknown pid;
- no directory/status files beyond `fd`.

## Blocker and Next Gate

Implementation is blocked until WinuxCmd source or a documented host API for virtual path providers, process registration, and native-handle-to-target mapping is available. The next agent should inspect WinuxCmd's real `readlink`, `test`, directory iterator, process table, and child-handle code, then implement the backend contract first. Only after those sources and BusyBox raw artifacts exist should Rubash add an integration test; do not add expected-output shims or a physical `proc` tree.
