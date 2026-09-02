# Process Substitution Temporary-File Path Separator Loss

> Investigation: 2026-09-01. Family J/K from DIFF-MASTER-PLAN.
> Rubash branch: fresh-master. Baseline: WSL GNU Bash 5.2.21.

## 1. Minimal Reproducer

**Script**: `target/issue-suites/results/probe/procsub-backslash-eval.sh`

```bash
#!/bin/bash
# Minimal reproducer: process substitution temp-file path backslash loss
# On GNU Bash, <(...) produces /dev/fd/N (forward slashes).
# On rubash (Windows), <(...) produces C:\Users\...\tmp\file.tmp (backslashes).
# When eval re-parses a word containing backslashes, they are consumed as
# escape characters, destroying the path.

echo "=== Test 1: eval echo <(echo hi) ==="
eval echo <(echo hi)

echo "=== Test 2: cat <(echo hello) (non-eval, works) ==="
cat <(echo hello)

echo "=== Test 3: eval cat <(echo test1) ==="
eval cat <(echo test1)

echo "=== Done ==="
```

**WSL GNU Bash 5.2.21 output** (baseline):

```
=== Test 1: eval echo <(echo hi) ===
/dev/fd/63
=== Test 2: cat <(echo hello) (non-eval, works) ===
hello
=== Test 3: eval cat <(echo test1) ===
test1
=== Done ===
```

**Rubash output** (bug reproduction):

```
=== Test 1: eval echo <(echo hi) ===
C:UsersAdministrator.dshtmprubash-process-subst-4224-1788224605568127300.tmp
=== Test 2: cat <(echo hello) (non-eval, works) ===
hello
=== Test 3: eval cat <(echo test1) ===

=== Done ===
target/.../procsub-backslash-eval.sh: line 1: cat: C:UsersAdministrator.dshtmp...
  rubash-process-subst-4224-1788224605575667300.tmp: No such file or directory
```

**Byte-level proof** (xxd of the path string from rubash):

```
00000000: 433a 5c55 7365 7273 5c41 646d 696e 6973  C:\Users\Adminis
00000010: 7472 6174 6f72 5c2e 6473 685c 746d 705c  trator\.dsh\tmp\
00000020: 7275 6261 7368 2d70 726f 6365 7373 2d73  rubash-process-s
00000030: 7562 7374 2d39 3438 342d 3137 3838 3232  ubst-9484-178822
00000040: 3433 3932 3230 3738 3139 3330 302e 746d  4392207819300.tm
00000050: 70                                       p
```

Literal `0x5C` (backslash) bytes at every path separator. After eval re-parsing,
these are consumed as escape characters:

```
Input:  C:\Users\Administrator\.dsh\tmp\rubash-process-subst-9484-...tmp
Output: C:UsersAdministrator.dshtmprubash-process-subst-9484-...tmp
        ^backslashes eaten^
```

---

## 2. GNU Source Evidence

**File**: `third_party/bash/subst.c`
**Function**: `process_substitute()` (line 6362)
**Key lines**: 6367–6394

GNU Bash uses `HAVE_DEV_FD` on Linux. The process substitution creates a pipe,
moves the parent end to a high fd, then builds the path via `make_dev_fd_filename()`:

```c
// subst.c:6367-6394
#if defined (HAVE_DEV_FD)
  if (pipe (fildes) < 0) { ... }
  parent_pipe_fd = fildes[open_for_read_in_child];
  parent_pipe_fd = move_to_high_fd (parent_pipe_fd, 1, 64);
  pathname = make_dev_fd_filename (parent_pipe_fd);  // "/dev/fd/N"
#endif
```

**File**: `third_party/bash/subst.c`
**Function**: `make_dev_fd_filename()` (line 6333)

```c
static char *
make_dev_fd_filename (int fd)
{
  char *ret, intbuf[INT_STRLEN_BOUND (int) + 1], *p;
  ret = (char *)xmalloc (sizeof (DEV_FD_PREFIX) + 8);
  strcpy (ret, DEV_FD_PREFIX);        // "/dev/fd/"
  p = inttostr (fd, intbuf, sizeof (intbuf));
  strcpy (ret + sizeof (DEV_FD_PREFIX) - 1, p);  // "/dev/fd/63"
  add_fifo_list (fd);
  return (ret);
}
```

`DEV_FD_PREFIX` is `"/dev/fd/"` (configured by `configure`). The resulting path
is always `/dev/fd/N` — a pure forward-slash path that survives `eval`
re-parsing without any escape interpretation.

**Non-HAVE_DEV_FD path** (line 6378–6379): `make_named_pipe()` calls
`sh_mktmpname("sh-np", ...)` which builds `tdir/lroot-N` using the platform's
`/` separator (line 194 of `lib/sh/tmpfile.c`). Even without `/dev/fd`, GNU
produces forward-slash paths.

**The contract**: GNU Bash always produces forward-slash paths for process
substitution. This is not an accident — it is required by the fact that
process substitution paths get embedded into command words that may later go
through `eval` re-parsing.

---

## 3. Rust Owner

**File**: `src/executor/execution_misc.rs`
**Function**: `shell_display_path()` (line 310)
**Lines**: 310–317

```rust
pub(in crate::executor) fn shell_display_path(path: &str) -> String {
    if cfg!(windows) {
        let path = path.strip_prefix("//?/").unwrap_or(path);
        let path = crate::executor::path::shell_path_display_from_windows(path);
        return windows_native_to_slash_drive_display(&path);
    }
    path.to_string()
}
```

**What it does differently from GNU**:

1. The input `path` comes from `PathBuf::to_string_lossy()` in
   `process_substitution_temp_path()` (`external_setup.rs:677–696`). On Windows,
   `PathBuf` uses `\` as separator, so the input is
   `C:\Users\...\tmp\rubash-process-subst-....tmp`.

2. `shell_path_display_from_windows()` (`path.rs:799`) only replaces
   `WINDOWS_LITERAL_STAR` and `WINDOWS_LITERAL_QUESTION` — it does NOT convert
   `\` to `/`.

3. `windows_native_to_slash_drive_display()` (`execution_misc.rs:319`) checks
   for `bytes[2] == b'/'` to convert `C:/path` to `/c/path`. But the path has
   `\` at position 2, so this conversion does NOT fire.

4. **Result**: the path passes through unchanged as
   `C:\Users\Administrator\.dsh\tmp\....tmp` with literal backslash bytes.

**Downstream impact**: This function is called in ~30 places across
`external_setup.rs`, `pipeline_exec.rs`, and other executor modules to convert
process substitution paths (and redirect targets) into shell-displayable strings.
Every call site embeds the result into command words or redirect targets that may
be re-parsed by `eval`, `source`, or other re-parsing contexts.

---

## 4. Root Cause

GNU Bash's `process_substitute()` in `subst.c` builds a pathname via
`make_dev_fd_filename()`, which prepends `"/dev/fd/"` and appends the fd number.
This always yields a forward-slash path (e.g., `/dev/fd/63`). Even on systems
without `HAVE_DEV_FD`, the named-pipe path from `sh_mktmpname()` uses forward
slashes because the platform path separator on Unix is `/`. The resulting path
is substituted directly into the command word via `sub_append_string()`, and
because it contains only forward slashes, it survives any subsequent `eval`
re-parsing without backslash-escape interpretation.

Rubash's `process_substitution_temp_path()` creates a `PathBuf` using
`shell_path_to_windows()` and `.push()`, which on Windows produces a native
backslash-separated path (e.g., `C:\Users\Administrator\.dsh\tmp\....tmp`).
The `shell_display_path()` function is then called to convert this into a
shell-safe display string, but it only handles the `//?/` prefix strip and a
drive-letter prefix conversion (`C:/` → `/c/`). It does NOT convert internal
backslashes to forward slashes. Consequently, the literal `0x5C` bytes in the
path survive into the command word. When that word is later re-parsed by
`eval`, the backslash characters are interpreted as shell escape characters,
consuming the following character and destroying the path (e.g., `\U` → `U`,
`\A` → `A`, `\.` → `.`).

---

## 5. Proposed Source-Consistent Fix

**File**: `src/executor/execution_misc.rs`
**Function**: `shell_display_path()` (line 310)

**Before**:

```rust
pub(in crate::executor) fn shell_display_path(path: &str) -> String {
    if cfg!(windows) {
        let path = path.strip_prefix("//?/").unwrap_or(path);
        let path = crate::executor::path::shell_path_display_from_windows(path);
        return windows_native_to_slash_drive_display(&path);
    }
    path.to_string()
}
```

**After**:

```rust
pub(in crate::executor) fn shell_display_path(path: &str) -> String {
    if cfg!(windows) {
        let path = path.strip_prefix("//?/").unwrap_or(path);
        let path = crate::executor::path::shell_path_display_from_windows(path);
        let path = path.replace('\\', "/");
        return windows_native_to_slash_drive_display(&path);
    }
    path.to_string()
}
```

**Why this matches the C behavior**: GNU Bash always produces forward-slash
paths for process substitution (`/dev/fd/N`). This fix converts Windows
backslashes to forward slashes in the display path, matching the GNU contract
that process-substitution paths must be eval-safe. The conversion happens before
`windows_native_to_slash_drive_display`, so the drive-letter conversion
(`C:/` → `/c/`) now correctly fires because position 2 is `/` after the
replacement.

**Overlap with other owners**: This function is shared by all redirect-target and
process-substitution path display. The captain should verify that:
- All existing redirect-target tests still pass (the change makes paths use `/`
  instead of `\`, which is valid for Windows file I/O).
- The `procsub` upstream check improvement is measured.
- The `dstack` / pushd-popd tests (family F) that show Windows-path
  differences (`C:/...` vs `/usr`) may also shift.

---

## 6. Verification Plan

**Focused Rust test**: Add or update a test in the executor test suite (e.g.,
`tests/executor_command_chaining/`) that:

1. Sets up a process substitution in an eval context
2. Verifies the substituted path contains no literal backslash bytes
3. Verifies the eval re-parsed command receives a valid file path

**run-83.sh check**:

```
MSYS_NO_PATHCONV=1 wsl bash tests/gnu-compat/run-83.sh check procsub
```

This should produce fewer DIFF lines (currently 386 diff lines vs 24 expected
output lines). The specific improvements expected:
- Lines 1, 3, 4 of the rubash output (the `cat: C:Users...` and
  `command not found` errors) should become correct process-substitution
  reads.

**Additional targeted checks**:

```bash
MSYS_NO_PATHCONV=1 wsl bash tests/gnu-compat/run-83.sh check dstack
MSYS_NO_PATHCONV=1 wsl bash tests/gnu-compat/run-83.sh check redir
```

These verify that the backslash-to-slash conversion doesn't regress redirect
targets or path-display for other families.
