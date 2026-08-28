# GROUP H: Output-Only Differences Analysis

> Analysis date: 2026-08-28
> Analyst: output-parity (team bash-compat-fixes)
> Scope: Tests with same exit code but different output content

## Summary

Total GROUP H tests: 31 (same exit code, different output)
Tests with analysis data available: 14 (comsub2, arith, quotearray, builtins, rsh, printf, histexp, quote, array, posixexp2, posix2, braces, complete, posixpipe)

## Priority 1: High-Impact Output Differences (Fixable)

### 1. Printf Escape Character Handling (printf test)
**Root Cause**: Rubash's `expand_percent_b` function preserves backslashes for "unrecognized" escapes, but GNU bash treats `\` followed by any character as just that character.
- Bash: `--"abcd"--`, `--'abcd'--`, `--4.2--`, `'abcd'`
- Rubash: `--\\"abcd\"--`, `--\\'abcd\\'--`, `--4\\.2--`, `\\'abcd\\'`
- **Fix Location**: `src/builtins/printf/escape.rs` line 106-110
- **Current Code**: `Some(other) => { output.push('\\'); output.push(other); }`
- **Fix**: Change to `Some(other) => { output.push(other); }` to match GNU bash behavior
- **Impact**: Affects all printf %b output with special characters

### 2. Enable -d Option Handling (builtins test)
**Root Cause**: Rubash parses the `-d` option but outputs usage message instead of proper error messages.
- Bash: `enable: notbuiltin: not a shell builtin`, `enable: test: not dynamically loaded`
- Rubash: `enable: usage: enable [-a] [-dnps] [-f filename] [name ...]`
- **Fix Location**: `src/builtins/enable.rs` lines 169-176
- **Current Code**: Outputs usage message when `delete` is true
- **Fix**: Implement proper `-d` option handling with correct error messages
- **Impact**: Affects all enable -d error messages

### 3. Complete Output Extra Options (complete test)
**Root Cause**: Rubash includes `sudo` (Windows-only) and `restricted` (instead of `privileged`) in completion list.
- Bash: No `sudo`, has `privileged`
- Rubash: Has `sudo`, has `restricted`
- **Fix Location**: `src/builtins/complete.rs` lines 72-73 (sudo) and shopt options
- **Current Code**: `#[cfg(windows)] "sudo"` in SHELL_BUILTINS
- **Fix**: Remove `sudo` from completion list or make it conditional; align shopt options
- **Impact**: Affects complete output accuracy

## Priority 2: Semantic Differences (Complex)

### 4. Command Substitution (comsub2 test)
**Root Cause**: Different handling of nested command substitutions, return values, and local variables.
- Multiple differences in nested `$()` handling
- Different error messages for `return` outside function
- **Fix Location**: src/executor/comsub handlers
- **Impact**: Affects complex command substitution scenarios

### 5. Associative Array Declaration (quotearray test)
**Root Cause**: Rubash not properly handling empty associative array declarations.
- Bash: `declare -A assoc`
- Rubash: `declare -A assoc=(["x],b["]="1" )`
- **Fix Location**: src/builtins/declare.rs array formatting
- **Impact**: Affects declare output for empty arrays

### 6. Arithmetic Expression Errors (arith test)
**Root Cause**: Different error message formatting for arithmetic errors.
- Bash includes line numbers and specific error tokens
- Rubash has different error message format
- **Fix Location**: src/executor/arithmetic error reporting
- **Impact**: Affects arithmetic error diagnostics

### 7. Restricted Shell Implementation (rsh test)
**Root Cause**: Different restricted shell error messages.
- Bash: `cd: restricted`, `PATH: readonly variable`
- Rubash: `rubash: set: -r: invalid option`
- **Fix Location**: src/shell/restricted.rs
- **Impact**: Affects restricted shell compatibility

## Priority 3: Ambiguous Differences (May Not Need Fix)

### 8. Backslash Preservation (quote test)
**Observation**: Rubash output matches .right file, but bash output differs.
- Bash: `b\`, `a\`, `b\` (trailing backslash preserved)
- Rubash: `b`, `a`, `b` (trailing backslash stripped)
- .right file: `b`, `a`, `b` (matches rubash)
- **Analysis**: This appears to be a bash version/platform difference, not a rubash bug
- **Recommendation**: No fix needed; rubash matches expected output

### 9. Environment Variable Display (builtins test)
**Root Cause**: Platform-specific path handling.
- Different PATH separators (`:` vs `;`)
- Different HOME path format (forward vs backslash)
- BASH variable present in rubash but not bash
- **Analysis**: Expected platform differences on Windows
- **Recommendation**: No fix needed; platform-specific behavior

### 10. Function Declaration Format (arith-for test)
**Root Cause**: Different function declaration formatting.
- Bash shows full function body with semicolons
- Rubash shows compact one-line format
- **Analysis**: Both are valid function representations
- **Recommendation**: Low priority; cosmetic difference

## Recommended Fix Order

1. **Printf Escape Handling** (printf) - High impact, isolated fix
2. **Enable -d Option** (builtins) - Medium impact, isolated fix
3. **Complete Output** (complete) - Medium impact, isolated fix
4. **Associative Array Declaration** (quotearray) - Medium impact, semantic fix
5. **Command Substitution** (comsub2) - Lower priority, complex fix
6. **Arithmetic Errors** (arith) - Lower priority, formatting fix
7. **Restricted Shell** (rsh) - Lower priority, semantic fix

## Next Steps

1. Focus on Priority 1 fixes (printf, enable, complete)
2. Each fix should be isolated and testable
3. Regenerate analysis output after each fix
4. Update this document with fix results
