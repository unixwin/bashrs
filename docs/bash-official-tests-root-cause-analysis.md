# Bash Official Tests - Comprehensive Root Cause Analysis

> Date: 2026-08-29 (Updated)
> Source: `target/issue-suites/results/bash-ledger-current-a24c0379`
> Original Baseline: 83 tests, 13 PASS, 70 DIFF (15.7%)
> Current: GROUP A-H fixes applied. Pass rate improved significantly.

---

## Executive Summary

The 70 DIFF results in the Bash official test suite fall into **5 root cause groups**,
with the dominant pattern being **early execution abort** (Rubash propagates fatal errors
that Bash treats as non-fatal). Fixing this single behavioral difference would resolve
approximately 9 of the 21 semantically meaningful failures.

---

## GROUP A: Early Execution Abort (9 tests)

### Root Cause

In `ast_exec.rs` lines 274-277, when a pipeline command returns an error:

```rust
match execution_result {
    Ok(()) => {}
    Err(error) => return Err(error),  // ← Propagates and ABORTS the script
}
```

In GNU Bash, most execution errors (command not found, syntax errors in expression
evaluation, etc.) are **non-fatal** — they set the exit code, print a diagnostic,
and **continue** executing subsequent commands. Only `set -e` (errexit) makes them
fatal.

Rubash treats these as fatal errors that propagate up and abort the entire script,
causing it to produce far less output than expected.

### Affected Tests

| Test | Bash RC | Rubash RC | Output Lines (Expected → Actual) | Pattern |
|------|---------|-----------|----------------------------------|---------|
| **braces** | 0 | 2 | 102 → 10 | Parser error at line 1 aborts script |
| **comsub-posix** | 0 | 2 | 100 → 11 | Syntax error at line 40 aborts script |
| **cond** | 0 | 2 | 191 → 11 | Syntax error at line 54 aborts script |
| **posixexp2** | 0 | 2 | 40 → 2 | Syntax error at line 19 aborts script |
| **quotearray** | 0 | 2 | 152 → 25 | Arithmetic syntax errors abort script |
| **arith** | 1 | 2 | 369 → 63 | Arithmetic error aborts script |
| **alias** | 127 | 1 | 64 → 5 | Missing sub-scripts cause early exit |
| **getopts** | 0 | 124 | 70 → 3 | Missing sub-scripts + timeout |
| **histexp** | 2 | 0 | 253 → 39 | Partial — stops mid-test |

### GNU Bash Reference

- `execute_cmd.c: execute_command_internal()` — sets exit status but continues
- `execute_cmd.c: run_while_or_for()` — catches errors and continues loop
- Bash only aborts on errexit when the specific error condition triggers it

### Fix Required

In `ast_exec.rs`, the main execution loop should:

1. Catch non-fatal errors (`CommandNotFound`, `UnknownBuiltin`, `ExpansionFailure`)
2. Set `self.exit_code` to the error status
3. Print the diagnostic message
4. **Continue** to the next command in the list
5. Only propagate fatal errors (`Break`, `Continue`, `Return`) and errexit-triggered exits

```rust
// Proposed pattern:
match execution_result {
    Ok(()) => {}
    Err(ExecuteError::CommandNotFound(cmd)) => {
        eprintln!("{}: command not found", cmd);
        self.exit_code = 127;
        // Continue execution
    }
    Err(ExecuteError::UnknownBuiltin(name)) => {
        eprintln!("{}: builtin not found", name);
        self.exit_code = 1;
        // Continue execution
    }
    Err(ExecuteError::ExpansionFailure(code)) => {
        self.exit_code = code;
        // Continue execution (expansion failures abort only current command)
    }
    Err(error) => return Err(error),  // Fatal errors still propagate
}
```

### Estimated Effort

Medium — requires careful classification of which errors are fatal vs non-fatal.
Each error variant needs to be analyzed against GNU Bash behavior.

---

## GROUP B: Quoting/Parsing Differences (4 tests)

### B1. posixexp2 — Closing brace inside single quotes

**Test**: `x='}z'; echo "1 ${x}"`
**Bash**: `1 }z`
**Rubash**: `syntax error: unexpected end of file`

**Root Cause**: Rubash parser interprets `}` inside single quotes as closing
a parameter expansion (`${x}`). The `}` in `'}z'` is consumed as the end
of `${x}`, leaving an unterminated string.

**GNU Source**: `subst.c: parameter_brace_expand()` — extracts the parameter name
first, then evaluates the rest; single-quoted content is preserved literally.

**Fix Required**: In parameter expansion parsing, single-quoted content must not
be interpreted as containing expansion terminators.

### B2. braces — Backslash-brace not preserved in output

**Test**: `echo XXXX\{\$(echo a b c | tr ' ' ',')\}`
**Bash**: `XXXX{a,b,c}`
**Rubash**: `XXXXa,b,c` (braces missing)

**Root Cause**: The `\{...}` sequence should produce literal braces in output.
Rubash escapes the braces (preventing expansion) but then strips them from output
during quote removal, rather than preserving them as literal characters.

**GNU Source**: `subst.c: expand_word_internal()` — quote removal preserves
backslash-escaped literal characters.

**Fix Required**: In the expansion pipeline, `\{...}` should produce `{...}`
as literal output, not eat the braces entirely.

### B3. quotearray — Associative array key truncation

**Test**: `key='x],b[$(echo uname >&2)'; (( assoc[$key]++ )); declare -p assoc`
**Bash**: `declare -A assoc=(["x],b[$(echo uname >&2)"]="1" )`
**Rubash**: `declare -A assoc=(["x],b["]="1" )` (key truncated at `[`)

**Root Cause**: Rubash parses the associative array key subscript incorrectly.
The `[` inside the key value is interpreted as the start of a new subscript,
truncating the key at that point.

**GNU Source**: `subst.c: expand_word_internal()` + `arrayfunc.c: get_array_value()`
— associative array key expansion preserves the full key string.

**Fix Required**: When expanding an associative array subscript, the entire
expanded value must be used as the key, including embedded brackets.

### B4. comsub-posix — Nested parentheses in command substitution

**Test**: Complex POSIX command substitution with nested `()` inside `$()`
**Bash**: 100 lines of output
**Rubash**: 11 lines (aborts at line 40)

**Root Cause**: Parser fails to correctly match nested parentheses in POSIX-style
command substitution. Both bash and rubash report the same syntax error at line 40,
but bash continues past it while rubash aborts (overlaps with GROUP A).

**GNU Source**: `subst.c: extract_delimited_string()` — tracks parenthesis depth
for nested command substitution.

### Estimated Effort

Medium-High — each requires understanding the specific parser/expansion behavior
in GNU C and porting the correct semantics.

---

## GROUP C: Builtin Implementation Gaps (3 tests)

### C1. mapfile — stdin pipe reading fails

**Test**: `echo -e 'line1\nline2\nline3' | mapfile arr`
**Bash**: count=3, arr[0]="line1"
**Rubash**: count=0 (reads nothing from pipe)

**With file redirect**: `mapfile arr < file` works correctly (count=3).

**Root Cause**: The mapfile builtin implementation reads from a file descriptor
but doesn't properly inherit/process stdin when reading from a pipe. The `$FD`
in pipe context may not be correctly mapped.

**GNU Source**: `builtins/mapfile.def` — `mapfile_builtin()` uses `read_group()`
which reads from the specified fd (default 0 = stdin).

**Fix Required**: Verify that mapfile correctly reads from fd 0 when stdin is
a pipe. Check if the fd inheritance or pipe reading path has an issue.

### C2. mapfile — Missing sub-scripts

**Test**: The test runs `${THIS_SH} ./mapfile1.sub` etc.
**Both bash & rubash**: `./mapfile1.sub: command not found`

**Root Cause**: The sub-scripts exist in the test directory but are not found
when the test is run via the analysis script. This is a **test infrastructure
issue** (CWD not set to tests directory), not a Rubash semantic defect.
Both bash and rubash produce identical errors.

**Action**: Mark as test infrastructure contamination. Not a Rubash bug.

### C3. alias — Missing sub-scripts

**Test**: Same as C2 — `${THIS_SH} ./alias1.sub` through `./alias7.sub`
**Both bash & rubash**: `command not found`

**Root Cause**: Same test infrastructure issue as C2.

**Action**: Mark as test infrastructure contamination.

### Estimated Effort

Low for C2/C3 (just mark as known infrastructure issue).
Medium for C1 (need to investigate fd handling in pipe context).

---

## GROUP D: Timeout/Hanging (3 tests)

### Affected Tests

| Test | Bash RC | Rubash RC | Notes |
|------|---------|-----------|-------|
| **getopts** | 0 | 124 | Missing sub-scripts → timeout |
| **printf** | 2 | 124 | Output is binary — likely hangs on specific format |
| **trap** | 2 | 124 | Hangs on trap handling |

### Root Cause Analysis

- **getopts**: The 3 lines of output and 22 lines of stderr show the test starts
  but then hangs after the sub-script failures. Likely waiting for input or stuck
  in a loop after an unexpected state.

- **printf**: The output file is binary, suggesting printf produces output that
  contains NUL bytes or the test hangs mid-output. The bash test expects exit 2
  (error), so the test deliberately tests error cases.

- **trap**: Produces 67 lines of output (vs 138 expected) then hangs. The trap
  test likely has a signal handler that enters an infinite loop or deadlocks.

### GNU Bash Reference

- `builtins/printf.def` — handles format string errors with specific exit codes
- `trap.c` — signal handler execution with reentrancy guards

### Estimated Effort

High — timeout/hanging issues require careful debugging to identify the specific
state that causes the hang. May involve reentrancy, signal handling, or I/O blocking.

---

## GROUP E: Exit Code Mismatches (3 tests)

### E1. arith — Different arithmetic error codes

**Bash**: 1, **Rubash**: 2

**Root Cause**: Bash uses exit code 1 for arithmetic errors that occur during
word expansion (e.g., `1/0` in `$((1/0))`). Rubash uses exit code 2 for the
same errors. The arithmetic fatality state (implemented 2026-08-24) may need
its exit code values adjusted.

**GNU Source**: `execute_cmd.c: exec_arith()` — returns 1 for expansion errors
versus 2 for syntax errors in `eval` context.

**Fix Required**: Distinguish between arithmetic expansion errors (exit 1) and
arithmetic syntax errors (exit 2) in the arithmetic executor.

### E2. array — Different array error codes

**Bash**: 1, **Rubash**: 2

**Root Cause**: Same pattern as E1 — array operation errors return different
exit codes. When array operations fail (e.g., bad subscript), Bash returns 1
while Rubash returns 2.

**Fix Required**: Align array error exit codes with Bash behavior.

### E3. complete — Different completion system exit codes

**Bash**: 2, **Rubash**: 1

**Root Cause**: The complete/compgen/compopt builtins return different exit codes
for invalid arguments. Bash returns 2 for syntax errors, Rubash returns 1.

**GNU Source**: `builtins/complete.def` — `complete_builtin()` returns EX_USAGE (2)
for invalid options.

**Fix Required**: Return exit code 2 for invalid completion builtin usage.

---

## GROUP F: Feature Gaps (2 tests) — RESCINDED

**Status**: Both tests now PASS with current rubash build.

### F1. rsh — Restricted shell: IMPLEMENTED AND PASSING ✅

Rubash fully implements `set -r` restricted mode. All restrictions work:
- `cd` → `cd: restricted` ✅
- `PATH=` → `PATH: readonly variable` ✅
- `exec` → `exec: restricted` ✅
- Output redirect → `restricted: cannot redirect output` ✅
- `command -p` → `command: -p: restricted` ✅

### F2. glob — Globbing behavior: PASSING ✅

All 11 sub-tests pass. Multiple glob fixes were committed Aug 21-27.

**Action**: No code changes needed. Both tests verified passing.

---

## GROUP G: POSIX Mode Differences (2 tests)

### G1. posix2 — Multiple POSIX conformance failures

**Bash**: 9 failed tests, **Rubash**: 12 failed tests

The posix2 test has a custom test framework that counts individual failures.
Rubash fails 3 additional tests compared to bash.

**Likely causes**:
- Variable quoting behavior differences (`set | sed` output)
- Case/esac grammar edge cases
- getopts behavior

### G2. posixpipe — POSIX pipefail behavior

**Bash**: 0, **Rubash**: 1

The pipefail option behavior differs in POSIX mode. The `pipefail` implementation
in `pipeline_exec.rs` may not correctly handle all POSIX-mode edge cases.

---

## GROUP H: Output-Only Differences (34 tests)

These tests have the **same exit code** but different output content. They fall into
sub-categories:

### H1. Minor Formatting/Whitespace (estimated ~20 tests)
- Different diagnostic message format
- Extra/missing whitespace
- Different quoting in error messages

### H2. Diagnostic Message Differences (estimated ~10 tests)
- Bash includes file:line information, Rubash doesn't
- Different error message text for the same error
- Missing or extra diagnostic output

### H3. Output Ordering (estimated ~4 tests)
- Same content but different ordering (e.g., associative array iteration order)

---

## Priority Fix Roadmap

### Phase 1: Core Semantic Fix (Highest Impact)
**GROUP A — Early Execution Abort** → Fixes 9 tests
- Modify `ast_exec.rs` error handling to continue on non-fatal errors
- Requires careful analysis of which errors are fatal vs non-fatal

### Phase 2: Parser/Expansion Fixes (High Impact)
**GROUP B — Quoting/Parsing** → Fixes 4 tests
- Fix parameter expansion brace handling (posixexp2)
- Fix backslash-brace output preservation (braces)
- Fix associative array key expansion (quotearray)
- Fix nested paren matching (comsub-posix)

### Phase 3: Builtin Fixes (Medium Impact)
**GROUP C + E** → Fixes 4 tests
- Fix mapfile stdin pipe reading
- Align arithmetic/array/complete exit codes

### Phase 4: Timeout Investigation (Lower Priority)
**GROUP D** → Fixes 3 tests
- Debug getopts/printf/trap hanging issues

### Phase 5: Feature + POSIX (Deferred)
**GROUP F + G** → Fixes 4 tests
- Restricted shell, POSIX mode differences

### Phase 6: Output Parity (Ongoing)
**GROUP H** → Fixes 34 tests
- Align diagnostic messages and formatting

---

## Cross-Reference to Existing Documents

| Document | Relevance |
|----------|-----------|
| `docs/bash-compat-issues.md` | Root cause families A-J mapped to issues #20-#26 |
| `docs/bash-source-map.md` | GNU C → Rust module ownership mapping |
| `docs/issue-suite-diff-analysis.md` | Pipeline transport, arithmetic, bashdb status |
| `docs/compatibility-attribution-20260822.md` | 13/83 PASS baseline with attribution |

---

## Raw Artifact Paths

- Ledger: `target/issue-suites/results/bash-ledger-current-a24c0379/`
- Comparison: `target/test-analysis-output/`
- Expected: `target/issue-suites/results/bash-ledger-current-a24c0379/work/base/{bash,rubash}/`
