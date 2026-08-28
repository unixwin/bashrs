# GROUP H Output Differences Summary

> Analysis date: 2026-08-28
> Analyst: output-parity (team bash-compat-fixes)

## Key Patterns Identified

### 1. Builtins Test (231 stdout differences, 5 stderr differences)

**Root Causes:**
- **Environment Variable Ordering**: Rubash outputs environment variables in insertion order, bash outputs them alphabetically
- **Environment Variable Values**: Different path formatting (forward vs backslash), different BASH variable
- **Enable Error Messages**: Rubash shows usage message instead of proper error for non-existent builtins
- **Error Message Formatting**: Different error messages for cp, rm operations

**Specific Issues:**
1. Line 45: Bash outputs "xxx", rubash outputs "bar" (different test variable)
2. Lines 94-112: Environment variable ordering differs
3. Line 115: Rubash has extra "restricted" option in set options
4. Stderr Lines 1-2: Enable error messages differ
5. Stderr Lines 6-8: cp/rm error messages differ

### 2. Complete Test (207 stdout differences, 0 stderr differences)

**Root Causes:**
- **Extra Shopt Option**: Rubash has "restricted" in set options where bash has "privileged"
- **Option Ordering**: Rubash has "restricted_shell" where bash has "privileged" in shopt list
- **Cascade Effect**: Extra option causes all subsequent options to shift down by one line

**Specific Issues:**
- Line 115: Bash has "verbose", rubash has "restricted"
- Line 116: Bash has "vi", rubash has "verbose"
- Line 117: Bash has "xtrace", rubash has "vi"
- (All subsequent lines are shifted)

### 3. Quote Test (3 stdout differences, 0 stderr differences)

**Root Causes:**
- **Backslash Preservation**: Bash preserves trailing backslashes, rubash doesn't
- **Escape Sequence Handling**: Different handling of escape sequences in certain contexts

**Specific Issues:**
- Line 49: Bash outputs "b\", rubash outputs "b"
- Line 50: Bash outputs "a\", rubash outputs "a"
- Line 51: Bash outputs "b\", rubash outputs "b"

## Recommended Fixes

### Priority 1: Complete Test (High Impact)
1. **Fix set options list**: Remove "restricted" from set options or add "privileged" to match bash
2. **Fix shopt options list**: Add "privileged" to shopt options or remove "restricted_shell"

### Priority 2: Builtins Test (Medium Impact)
1. **Fix enable error messages**: Implement proper error messages for non-existent builtins
2. **Fix error message formatting**: Align cp/rm error messages with bash

### Priority 3: Quote Test (Low Impact)
1. **Investigate backslash handling**: Understand why trailing backslashes are stripped
2. **Fix escape sequence handling**: Align with bash behavior

## Implementation Notes

### Complete Test Fix
The issue is in `src/builtins/set/options.rs` and `src/builtins/shopt/support.rs`:
- `set/options.rs` has "restricted" at line 117, but bash doesn't have this in set options
- `shopt/support.rs` has "restricted_shell" at line 60, but bash has "privileged" at line 114

### Builtins Test Fix
The issue is in `src/builtins/enable.rs`:
- Lines 169-176: Outputs usage message when `delete` is true
- Should implement proper `-d` option handling with correct error messages

### Quote Test Fix
The issue is likely in escape sequence handling in the executor or lexer.
Need to investigate how trailing backslashes are processed.
