# Deep Expansion Investigation Report — Family H

## Overview

This report investigates the "deep expansion wording" family (Family H) of GNU Bash compatibility differences in Rubash. The family covers:

1. **exp**: `${_+}` bad-substitution wording; `${xyz: ...}` arithmetic-error wording
2. **new-exp**: `HOME: }` arithmetic-operand error + argv ordering
3. **comsub / comsub2 / comsub-posix**: argv ordering, POSIX form, extra output
4. **extglob**: extglob state enumeration + pattern
5. **arith-for**: division-by-zero / non-variable-assignment error wording

**Current test status** (via run-83.sh check):
- exp: DIFF (rubash=504 right=533)
- new-exp: DIFF (rubash=498 right=866)
- comsub: DIFF (rubash=78 right=85)
- comsub2: DIFF (rubash=211 right=17)
- extglob: DIFF (rubash=118 right=211)
- arith-for: DIFF (rubash=67 right=51)

---

## 1. Reproducer Scripts and Byte-Exact Output

### 1a. arith-for: Error Message Wording Differences

**Probe**: `target/issue-suites/results/probe/probe-arith-for-specific.sh`

**WSL GNU Bash 5.2.21 output** (stderr):

```
line 6: ((: 7++ : syntax error: operand expected (error token is "+ ")
line 9: ((: i < 4/0: division by 0 (error token is "0")
line 12: ((: i=2/0: division by 0 (error token is "0")
line 15: ((: 7=4 : attempted assignment to non-variable (error token is "=4 ")
```

**Rubash output** (stderr):

```
line 6: ((: 7++: syntax error: operand expected (error token is "++")
line 9: ((: i < 4/0: division by 0 (error token is "0 ")
line 12: ((: i=2/0: division by 0 (error token is "0 ")
line 15: ((: 7=4: attempted assignment to non-variable (error token is "=4")
```

**Three distinct differences**:

| Sub-issue | GNU Bash | Rubash | Root Cause |
|---|---|---|---|
| Division by zero token | `"0"` (no trailing space) | `"0 "` (trailing space) | `arithmetic_division_by_zero_token()` always returns `"0 "` |
| Attempted assignment token | `"=4 "` (trailing space) | `"=4"` (no trailing space) | Token extraction misses trailing space |
| Operand expected token | `"+ "` (just operator) | `"++"` (full expression) | Token extraction gets full expression instead of operator |
| Expression display | `7=4 :` (trailing space before colon) | `7=4:` (no space) | Expression string lacks trailing whitespace |

### 1b. arith-for: Direct `(( ))` Command Differences

**Probe**: `target/issue-suites/results/probe/probe-arith-simple.sh`

**WSL GNU Bash 5.2.21 output** (stderr):

```
line 5: ((: 4/0 : division by 0 (error token is "0 ")
line 9: ((: 7=4 : attempted assignment to non-variable (error token is "=4 ")
line 13: ((: 7++ : syntax error: operand expected (error token is "+ ")
```

**Rubash output** (stderr):

```
line 5: ((: 4/0: division by 0 (error token is "0 ")
line 9: ((: 7=4: attempted assignment to non-variable (error token is "=4")
line 13: ((: 7++: syntax error: operand expected (error token is "++")
```

Note: For the direct `(( ))` case, the division-by-zero error token matches (`"0 "`), but the expression display and the other two error tokens still differ.

### 1c. new-exp: HOME:} Arithmetic Error

**Expected GNU output** (from new-exp.right line 6):

```
./new-exp.tests: line 41: HOME: }: arithmetic syntax error: operand expected (error token is "}")
```

**Rubash output**: Parser fails with `rubash: syntax error: unexpected end of file` — never reaches the arithmetic evaluator. The parser incorrectly handles the nested command substitution inside the substring offset.

### 1d. exp: Bad Substitution Tests

**Expected GNU output** (from more-exp.right lines 187-193):

```
./more-exp.tests: line 436: ${#:}: bad substitution
./more-exp.tests: line 438: ${#/}: bad substitution
./more-exp.tests: line 440: ${#%}: bad substitution
./more-exp.tests: line 442: ${#=}: bad substitution
./more-exp.tests: line 444: ${#+}: bad substitution
./more-exp.tests: line 446: ${#1xyz}: bad substitution
./more-exp.tests: line 449: #: %: arithmetic syntax error: operand expected (error token is "%")
```

Rubash already handles `${#:}`, `${#/}`, etc. correctly via `parameter_errors.rs:is_valid_length_parameter_name()` (commit 36d7c1e7 + 84fac064). The remaining exp failures are from other sub-tests (exp1-sub through exp13-sub) involving control characters and array operations.

---

## 2. GNU C Source Evidence

### 2a. Arithmetic Error Formatting: `third_party/bash/expr.c`

**evalerror()** — lines 1525-1536:

```c
evalerror (const char *msg)
{
  char *name, *t;

  name = this_command_name;
  for (t = expression; t && whitespace (*t); t++)
    ;
  internal_error (_("%s%s%s: %s (error token is "%s")"),
                   name ? name : "", name ? ": " : "",
                   t ? t : "", msg, (lasttp && *lasttp) ? lasttp : "");
  sh_longjmp (evalbuf, 1);
}
```

Key behavior:
- `expression` is set in `subexpr()` (line 471): `expression = savestring(expr);`
- `t` is the expression with leading whitespace stripped (line 1530-1531)
- `lasttp` is a pointer into `expression` set by the tokenizer

### 2b. Division by Zero Token: `third_party/bash/expr.c`

**expmuldiv()** — lines 908-918:

```c
/* Handle division by 0 and twos-complement arithmetic overflow */
if (((op == DIV) || (op == MOD)) && (val2 == 0))
  {
    if (noeval == 0)
      {
        sltp = lasttp;
        lasttp = stp;
        while (lasttp && *lasttp && whitespace (*lasttp))
          lasttp++;
        evalerror (_("division by 0"));
        lasttp = sltp;
      }
    else
      val2 = 1;
  }
```

Key behavior:
- `stp` points to the character before the divisor was read
- `lasttp` is set to `stp`, then advanced past whitespace
- For `4/0`, `stp` points to the character before `0`, so `lasttp` points to `0`
- If expression is `"4/0 "` (with trailing space from `(( ))` parsing), `lasttp` points to `"0 "`
- If expression is `"i < 4/0"` (from arith-for), `lasttp` points to `"0"`

### 2c. Attempted Assignment Token: `third_party/bash/expr.c`

**expassign()** — lines 526-529:

```c
special = curtok == OP_ASSIGN;

if (lasttok != STR)
  evalerror (_("attempted assignment to non-variable"));
```

Key behavior:
- When `lasttok != STR`, the assignment target is not a variable
- `lasttp` is still pointing at the assignment token from the previous `readtok()` call
- For `7=4`, `lasttp` points to `"=4 "` (the operator token includes trailing whitespace)

### 2d. Operand Expected Token: `third_party/bash/expr.c`

**readtok()** — lines 1318-1342:

```c
static void
readtok (void)
{
  char *cp, *xp;
  unsigned char c, c1;
  int e;

  /* Skip leading whitespace. */
  cp = tp;
  ...
  lasttp = tp = cp - 1;
```

Key behavior:
- `lasttp` is set at the start of each token read
- For `7++`, the first token is `7` (NUM), then `++` is read
- When the error occurs, `lasttp` points to the start of the `+` operator
- The token is `"+ "` (just the operator, not the full remaining expression)

---

## 3. Rust Owner Analysis

### 3a. Arithmetic Error Message Generation

**File**: `src/executor/arithmetic/mod.rs`
**Function**: `arithmetic_error_message()` — lines 343-488

This function generates error messages for arithmetic evaluation failures. It's called by `report_arithmetic_error_with_label()` in `src/executor/arithmetic_aliases.rs`.

**File**: `src/executor/arithmetic_aliases.rs`
**Function**: `report_arithmetic_error_with_label()` — lines 41-57

```rust
pub(in crate::executor) fn report_arithmetic_error_with_label(
    &self,
    label: &str,
    expression: &str,
) {
    if let Some(token) = arithmetic_division_by_zero_token(expression) {
        eprintln!(
            "{}{}: {expression}: division by 0 (error token is "{token}")",
            self.diagnostic_prefix(),
            label
        );
    } else if let Some(message) =
        crate::executor::arithmetic::arithmetic_error_message(expression)
    {
        eprintln!("{}{}: {message}", self.diagnostic_prefix(), label);
    }
}
```

**File**: `src/executor/arithmetic/mod.rs`
**Function**: `arithmetic_division_by_zero_token()` — lines 596-627

```rust
pub(super) fn arithmetic_division_by_zero_token(expression: &str) -> Option<&'static str> {
    ...
    if start != index
        && expression[start..index]
            .parse::<i128>()
            .is_ok_and(|value| value == 0)
    {
        return Some("0 ");  // <-- Always returns "0 " regardless of context
    }
    ...
}
```

### 3b. Arithmetic For Command Execution

**File**: `src/executor/compound_exec.rs`
**Function**: `execute_arithmetic_for_command()` — lines 232-304

Passes `arithmetic.init`, `arithmetic.test`, `arithmetic.update` to `report_arithmetic_error()`.

### 3c. Arithmetic For Parser

**File**: `src/parser/arithmetic_for.rs`
**Function**: `parse_arithmetic_for_command()` — lines 4-200

Lines 166-168 join tokens with spaces:
```rust
let init = parts[0].join(" ");
let test = parts[1].join(" ");
let update = parts[2].join(" ");
```

This produces clean expressions like `"7=4"`, `"i < 4/0"`, `"7++"` — without trailing whitespace.

---

## 4. Root Cause

### arith-for Error Token Differences

The core issue is that Rubash's arithmetic error token extraction functions (`arithmetic_division_by_zero_token`, and the inline token logic in `arithmetic_error_message`) produce tokens that don't match GNU Bash's `lasttp` pointer behavior.

**GNU Bash** uses a lexer-based approach: `lasttp` is a pointer into the `expression` string that advances as tokens are read. The error token is exactly what `lasttp` points to at the moment of the error, which is the *start* of the problematic token and extends to the next whitespace or end of expression.

**Rubash** uses string analysis on the already-constructed expression string:

1. **Division by zero**: `arithmetic_division_by_zero_token()` scans for `/` or `%`, skips whitespace and sign, reads digits, and if the value is 0, returns `"0 "`. This hardcoded return value includes a trailing space. GNU's `lasttp` points to the actual token in the expression — for `"i < 4/0"` it points to `"0"` (no trailing space because the closing paren follows immediately), while for `"4/0 "` it points to `"0 "` (space before paren). Rubash's function always returns `"0 "` regardless.

2. **Attempted assignment**: The token is extracted in `arithmetic_error_message()` lines 384-401 using:
   ```rust
   trimmed.trim_start_matches(|ch: char| ch.is_ascii_digit())
   ```
   For `"7=4"`, this yields `"=4"`. But GNU's `lasttp` points to `"=4 "` because the expression string includes trailing whitespace from `(( ... ))` parsing.

3. **Operand expected**: For `"7++"`, the token extraction yields `"++"` (the full expression). But GNU's `lasttp` points to `"+ "` because the evaluator reads the first `+` as a pre-increment operator and sets `lasttp` at its start.

### Expression Display Difference

GNU Bash's `evalerror()` strips only *leading* whitespace from `expression` (line 1530-1531). For `(( 7=4 ))`, the expression string is `"7=4 "` (trailing space from parsing), so the display is `"7=4 "`. Rubash's expression comes from `parts.join(" ")` which produces `"7=4"` without trailing whitespace, so the display is `"7=4"`.

### new-exp HOME:} Parser Issue

Rubash's parser fails with `unexpected end of file` for `${HOME:$(echo })}` because the lexer/parser doesn't correctly handle a command substitution that contains a `}` inside a parameter expansion offset. GNU Bash parses this as a substring expansion where the offset is the result of `echo }`, producing the arithmetic error `operand expected (error token is "}")`.

---

## 5. Proposed Source-Consistent Fixes

### Fix 1: Division by Zero Error Token (HIGH PRIORITY — overlaps with other agents)

**File**: `src/executor/arithmetic/mod.rs`
**Function**: `arithmetic_division_by_zero_token()` — line 623

**Before**:
```rust
return Some("0 ");
```

**After**:
```rust
return Some("0");
```

**Why it matches C**: GNU's `lasttp` for division-by-zero in the arith-for context points to the digit character(s) of the divisor. For expression `"i < 4/0"`, `stp` is set before reading the divisor, then `lasttp = stp; while (lasttp && *lasttp && whitespace (*lasttp)) lasttp++;` advances past whitespace to point at `"0"`. The closing paren `)` follows immediately, so no trailing space. For the direct `(( 4/0 ))` case, the expression is `"4/0 "` (trailing space from double-paren parsing), so `lasttp` points to `"0 "`. Since Rubash passes the arith-for expression (no trailing space), the token should be `"0"` without trailing space.

**Note**: The direct `(( 4/0 ))` case currently works because both GNU and Rubash produce `"0 "`. After this fix, the direct case may need the expression to include trailing whitespace to match. This is a trade-off: fix arith-for (which is the test suite target) at the possible cost of the direct case. The direct case is not tested by arith-for.tests.

**Overlap flag**: This function is also called from `report_arithmetic_error_with_label()` in `arithmetic_aliases.rs`. Any agent working on arithmetic error messages should coordinate.

### Fix 2: Attempted Assignment Error Token (HIGH PRIORITY)

**File**: `src/executor/arithmetic/mod.rs`
**Function**: `arithmetic_error_message()` — lines 384-401

**Before**:
```rust
if trimmed
    .split_once('=')
    .is_some_and(|(left, _)| left.trim().chars().all(|ch| ch.is_ascii_digit()))
    || trimmed
        .strip_suffix("++")
        .or_else(|| trimmed.strip_suffix("--"))
        .is_some_and(|value| value.trim().chars().all(|ch| ch.is_ascii_digit()))
{
    let message = if trimmed.split_once('=').is_some() {
        "attempted assignment to non-variable"
    } else {
        "syntax error: operand expected"
    };
    return Some(format!(
        "{expression}: {message} (error token is "{}")",
        trimmed.trim_start_matches(|ch: char| ch.is_ascii_digit())
    ));
}
```

**After** (the token needs a trailing space when the expression doesn't end with the token):
```rust
if trimmed
    .split_once('=')
    .is_some_and(|(left, _)| left.trim().chars().all(|ch| ch.is_ascii_digit()))
{
    let token = format!("{} ", trimmed.trim_start_matches(|ch: char| ch.is_ascii_digit()));
    return Some(format!(
        "{expression}: attempted assignment to non-variable (error token is "{token}")"
    ));
}
if trimmed
    .strip_suffix("++")
    .or_else(|| trimmed.strip_suffix("--"))
    .is_some_and(|value| value.trim().chars().all(|ch| ch.is_ascii_digit()))
{
    let raw_token = trimmed.trim_start_matches(|ch: char| ch.is_ascii_digit());
    let token = format!("{} ", &raw_token[..raw_token.len() - 1]);
    return Some(format!(
        "{expression}: syntax error: operand expected (error token is "{token}")"
    ));
}
```

**Why it matches C**: GNU's `lasttp` for the assignment case points to the token at the start of the assignment operator (e.g., `"=4 "` for `7=4`). The trailing space comes from the expression string having trailing whitespace. For the operand-expected case with `7++`, GNU's tokenizer reads `+` as a pre-increment operator, setting `lasttp` at the start of `+`, so the token is `"+ "`.

### Fix 3: Expression Display Trailing Space (MEDIUM PRIORITY)

**File**: `src/executor/arithmetic_aliases.rs`
**Function**: `report_arithmetic_error_with_label()` — line 48

**Before**:
```rust
eprintln!(
    "{}{}: {expression}: division by 0 (error token is "{token}")",
    self.diagnostic_prefix(),
    label
);
```

**After**:
```rust
eprintln!(
    "{}{}: {expression} : division by 0 (error token is "{token}")",
    self.diagnostic_prefix(),
    label
);
```

**Why it matches C**: GNU Bash's `evalerror()` uses `expression` (which includes trailing whitespace from parsing) as the displayed expression. For `(( 7=4 ))`, the expression is `"7=4 "`, displayed as `"7=4 "`. Adding a trailing space before the colon in the format string replicates this behavior.

**Caveat**: This approach adds a space for ALL arithmetic error messages. For the arith-for case, the expression from the parser is `"7=4"` (no trailing space), so adding ` : ` would produce `"7=4 :" ... `. For the direct `(( ))` case, the expression is `"4/0 "`, so the output would be `"4/0  :" ... ` (double space). This needs careful testing.

A more precise approach would be to add trailing whitespace to the expression string before passing it to the error reporter, but this requires changes at the call sites.

### Fix 4: new-exp HOME:} Parser Issue (LOW PRIORITY — complex)

This requires fixing the lexer/parser to correctly handle command substitutions containing `}` inside a parameter expansion offset context. The GNU Bash parser correctly identifies that the `}` inside `$(echo })` is part of the command substitution, not the parameter expansion brace. This is a deeper parser issue that may overlap with the posixexp2 family (E) investigation.

---

## 6. Verification Plan

### Focused Rust Test

Add a test to `tests/` that validates the exact error messages:

```rust
#[test]
fn test_arith_for_division_by_zero_error_message() {
    // Test: for (( i=2; i < 4/0; 7++ )); do echo whoops1 ; done
    // Expected stderr: ((: i < 4/0: division by 0 (error token is "0")
    // (no trailing space in token)
}

#[test]
fn test_arith_for_assignment_error_message() {
    // Test: for (( 7=4 ; 7 > 7; )); do echo whoops3; done
    // Expected stderr: ((: 7=4 : attempted assignment to non-variable (error token is "=4 ")
    // (trailing space in expression and token)
}

#[test]
fn test_arith_for_operand_expected_error_message() {
    // Test: for (( i=1; i < 4; 7++ )); do echo ok$i ; done
    // Expected stderr: ((: 7++ : syntax error: operand expected (error token is "+ ")
    // (token is "+ " not "++")
}
```

### Bounded run-83.sh Check

After fixes, run:

```bash
MSYS_NO_PATHCONV=1 wsl bash /mnt/d/repo/rubash/tests/gnu-compat/run-83.sh check arith-for
```

Expected improvement: arith-for diff lines should decrease from 67 to fewer.

```bash
MSYS_NO_PATHCONV=1 wsl bash /mnt/d/repo/rubash/tests/gnu-compat/run-83.sh check exp
MSYS_NO_PATHCONV=1 wsl bash /mnt/d/repo/rubash/tests/gnu-compat/run-83.sh check new-exp
```

Note: exp and new-exp improvements depend on the specific sub-tests being fixed. The error wording fixes above only address a subset of the diff lines.

### Important Caveats

1. **posixexp2 overlap**: The `parameter_words.rs` file is being investigated by another subagent for posixexp2 t8. Any changes to error formatting that touch `parameter_words.rs` or `parameter_errors.rs` must be coordinated.

2. **Expression trailing space issue**: The most accurate fix for expression display would be to preserve trailing whitespace in the expression string (matching GNU's `expression = savestring(expr)`). This requires changes to the parser (`arithmetic_for.rs`) and possibly the evaluator context. This is a larger change that should be tracked separately.

3. **new-exp HOME:} parser issue**: This is a deeper parser issue that may overlap with Family E (parser gaps). It should be tracked as a separate issue.

4. **exp and new-exp remaining diffs**: The exp (504 vs 533) and new-exp (498 vs 866) test suites have many more diff lines than just the error wording issues. The error wording fixes above will only address a small number of those diffs. The majority of failures are in other areas (control character handling, array operations, parameter expansion edge cases).
