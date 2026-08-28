# Bash Official Tests - Detailed Findings (2026-08-28)

## Analysis Method

Ran all 21 failing tests with both GNU Bash and Rubash, captured stdout/stderr for comparison.

## Key Findings

### 1. braces (Bash:0, Rubash:2)

**Issue**: Output difference in escape handling
- Rubash: `{abc,def}`
- Bash: `{abc\,def}`

**Root Cause**: Rubash's brace expansion doesn't preserve escaped commas correctly. When encountering `{abc\,def}`, Bash treats the backslash-comma as a literal comma within the brace expression, while Rubash may be removing the backslash.

**GNU Source**: braces.c - handle_quoted_brace_content()

**Fix Required**: Improve escape sequence handling in brace expansion parser

---

### 2. mapfile (Bash:0, Rubash:2)

**Issue**: Identical output but different exit codes
- Both produce same stdout
- Rubash exits with 2, Bash with 0

**Root Cause**: Likely an error in later part of the test script that Rubash handles differently. Need to check stderr and full test script.

**Investigation Needed**: Check what happens after line 1 of output

---

### 3. quotearray (Bash:0, Rubash:2)

**Issue**: Completely different output
- Rubash: 25 lines starting with `declare -A assoc=(["x],b["]="1" )`
- Bash: 20 lines starting with `declare -A assoc`

**Root Cause**: Rubash is incorrectly parsing or displaying associative array declarations with special characters in keys. The key `["x],b["]` suggests quote/bracket handling issues.

**GNU Source**: assoc.c, arrayfunc.c - associative array declaration and display

**Fix Required**: Fix associative array key parsing and declare output formatting

---

### 4. cond (Bash:0, Rubash:2)

**Issue**: Identical output but different exit codes
- Both produce same 11 lines of output
- Rubash exits with 2, Bash with 0

**Root Cause**: Similar to mapfile - likely a test script error that Rubash handles differently

---

### 5. arith (Bash:1, Rubash:2)

**Issue**: Same output and errors, but Rubash missing some error messages
- Both show same arithmetic results
- Rubash missing several error messages that Bash shows
- Both fail at line 191 with syntax error

**Root Cause**: Rubash's error reporting for arithmetic expressions is incomplete. It stops reporting errors earlier than Bash does.

**GNU Source**: expr.c - error reporting in arithmetic evaluation

---

## Pattern Analysis

### Category A: Output Differences (Need Parser Fixes)
- braces - escape handling
- quotearray - associative array parsing/display

### Category B: Same Output, Different Exit Codes (Need Error Handling Fixes)  
- mapfile
- cond
- arith (partially)

### Category C: Timeout Issues
- getopts
- printf
- trap

(To be analyzed when script completes)

## Next Steps

1. **Priority 1**: Fix braces escape handling (clear parser issue)
2. **Priority 2**: Fix quotearray associative array handling (parser + display)
3. **Priority 3**: Investigate exit code mismatches (may be test script errors vs real bugs)
4. **Priority 4**: Debug timeout issues

## Estimated Impact

- Fixing braces and quotearray could resolve 2-4 related tests
- Exit code fixes may resolve 5-6 tests if they're consistent patterns
- Timeout fixes are more complex and may require deeper investigation
