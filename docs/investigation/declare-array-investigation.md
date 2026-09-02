# Family G Investigation: declare / array / nameref / varenv Output Format

**Date**: 2026-09-01
**Investigator**: Subagent (READ-ONLY)
**Baseline**: WSL GNU Bash 5.2.21

---

## Summary of Sub-Issues

| Sub-Issue | Category | Root Cause Found | Fix Proposed |
|---|---|---|---|
| nameref | `\${v//c/x}` through nameref returns target name, not resolved value | **Yes** | **Yes** |
| assoc key ordering | `declare -A` output key order differs from GNU | Yes (cosmetic) | Noted (hash table ordering) |
| assoc `declare -Ai` arithmetic | Integer assoc values not evaluated in declare output | Investigated | Noted |
| array unset | `unset c[2]` — matches GNU in basic cases | Matches | N/A |
| varenv | `c=7: command not found` — assignment-as-command | Matches for basic cases | N/A |

---

## 1. Minimal Reproducer: nameref Pattern Substitution

This is the **highest-impact bug** in family G: `\${v//c/x}` where `v` is a nameref to `var` fails to resolve through the nameref.

### Script (`target/issue-suites/results/probe/nameref-subissue.sh`)

```bash
#!/bin/bash
# Test: nameref pattern substitution
var=abcde
x=var
declare -n v=var
# these two should display the same
echo \${!x//c/x}
echo \${v//c/x}
```

### WSL GNU Bash 5.2.21 output (byte-exact)

```
abxde
abxde
```

### Rubash output (byte-exact)

```
abxde
var
```

**Difference**: `\${v//c/x}` resolves to `var` (the nameref target name) instead of `abxde` (the substitution result through the resolved target value `abcde`).

### Extended probe (`target/issue-suites/results/probe/nameref-deep.sh`)

```bash
#!/bin/bash
var=abcde
declare -n v=var
echo "Test: \${v//c/x}"
echo "result: \${v//c/x}"
echo "declare -p v:"
declare -p v
echo "simple deref: \${v}"
echo "result: \${v}"
```

GNU output:
```
Test: abxde
result: abxde
declare -p v:
declare -n v="var"
simple deref: abcde
result: abcde
```

Rubash output:
```
Test: var
result: var
declare -p v:
declare -n v="var"
simple deref: abcde
result: abcde
```

Note: `\${v}` (simple deref) works correctly in rubash — only `\${v//...}` (pattern substitution through nameref) fails.

---

## 2. GNU Source Evidence

### File: `third_party/bash/subst.c`

**Function**: `parameter_brace_expand_word` (lines 7663–7880)

GNU Bash resolves namerefs through `find_variable()`:

```c
// subst.c line 7774
else if (var = find_variable (name))
  {
    if (var_isset (var) && invisible_p (var) == 0)
      {
        // ... retrieves var->value, which is the resolved target's value
        temp = var_get_value(var);
        // ... applies pattern substitution to temp
      }
  }
```

**Function**: `find_variable` in `variables.c` (lines 2363–2384)

```c
// variables.c line 2363
SHELL_VAR *
find_variable (const char *name)
{
  SHELL_VAR *v;
  int flags;
  last_table_searched = 0;
  flags = 0;
  if (expanding_redir == 0 && (assigning_in_environment || executing_builtin))
    flags |= FV_FORCETEMPENV;
  v = find_variable_internal (name, flags);
  if (v && nameref_p (v))       // <--- resolves nameref chain
    {
      v = find_variable_nameref (v);
      if (v == &nameref_maxloop_value)
        {
          internal_warning (_("%s: maximum nameref depth (%d) exceeded"), name, NAMEREF_MAX);
          return (0);
        }
    }
  return v;
}
```

GNU's `find_variable()` automatically follows the nameref chain via `find_variable_nameref()` (lines 2011–2047). When `\${v//c/x}` is expanded, `find_variable("v")` returns the `var` SHELL_VAR (with value `abcde`), so the pattern substitution operates on `abcde` and produces `abxde`.

---

## 3. Rust Owner

### File: `src/executor/expand_braced_replacement.rs`

**Function**: `expand_braced_replacement_parameter` (lines 4–83)

```rust
pub(in crate::executor) fn expand_braced_replacement_parameter(
    &self,
    name: &str,
) -> Option<String> {
    let (var_name, pattern, replacement, global) = parse_parameter_replacement(name)?;
    // ... pattern and replacement expansion ...
    if let Some(value) =
        self.indirect_replacement_parameter(var_name, &pattern, &replacement, global)
    {
        return Some(value);
    }
    // ... positional params handling ...
    if let Some(value) = self.array_element_parameter_value(var_name) {
        return Some(replace_parameter_pattern(&value, &pattern, &replacement, global));
    }
    // ... array [@]/[*] handling ...
    if is_shell_name(var_name) {
        return Some(
            self.dynamic_parameter_value(var_name)
                .or_else(|| self.env_vars.get(var_name).cloned())   // <--- BUG HERE
                .map(|value| replace_parameter_pattern(&value, &pattern, &replacement, global))
                .unwrap_or_default(),
        );
    }
    None
}
```

**What it does differently from C**: At line 77, `self.env_vars.get(var_name)` directly looks up the raw value of `var_name` in the HashMap. When `var_name` is `"v"` (a nameref), `env_vars.get("v")` returns `"var"` (the nameref target name string), not the resolved variable's value `abcde`. The pattern substitution then operates on the string `"var"` which doesn't contain the letter `c`, producing `"var"` unchanged.

GNU's `find_variable("v")` follows the nameref chain, returning the SHELL_VAR for `var` (with value `abcde`), so substitution operates on `abcde`.

### Related Rust function that does it correctly

**File**: `src/executor/parameter_patterns.rs`

**Function**: `parameter_pattern_scalar_value` (lines 131–151)

```rust
pub(in crate::executor) fn parameter_pattern_scalar_value(&self, name: &str) -> Option<String> {
    // ...
    let resolved = self.resolved_variable_name(name)?;    // <--- resolves nameref
    let value = self.env_vars.get(&resolved)?;
    // ...
}
```

This function correctly uses `resolved_variable_name()` which calls `nameref_resolution()` to follow the nameref chain. The pattern removal code (`\${v%...}`, `\${v#...}`) goes through `parameter_pattern_scalar_value` and therefore works correctly with namerefs.

### Owner overlap

The nameref resolution infrastructure lives in:
- `src/executor/variable_state.rs` — `nameref_resolution()`, `nameref_target_name()`, `resolved_variable_name()` (lines 39–61, 24–37)
- `src/executor/expand_braced_replacement.rs` — the buggy file (needs fix)
- `src/executor/expand_braced_patterns.rs` — correctly delegates to `parameter_pattern_scalar_value` which resolves namerefs

**No overlap with lexer/continuation.rs** (captain-exclusive).

---

## 4. Root Cause

In GNU Bash's `subst.c`, the function `parameter_brace_expand_word` (line 7774) resolves variable names through `find_variable()`, which automatically follows nameref chains via `find_variable_nameref()` (variables.c line 2373). This means that when `\${v//c/x}` is expanded where `v` is a nameref to `var`, GNU resolves `v` → `var` and retrieves the value `abcde`, then applies the pattern substitution to produce `abxde`.

In rubash's `expand_braced_replacement_parameter` (expand_braced_replacement.rs line 77), the final fallback path does a raw HashMap lookup `self.env_vars.get(var_name)` without resolving namerefs. When `var_name` is `"v"`, this returns the string `"var"` (the nameref target name, which is the raw stored value), not the resolved variable's actual value `abcde`. The pattern substitution then operates on the string `"var"` — which contains no `c` — and returns `"var"` unchanged.

The pattern removal functions (`\${v%...}`, `\${v#...}`) don't have this bug because they delegate to `parameter_pattern_scalar_value()` which correctly calls `self.resolved_variable_name(name)`. Only the pattern substitution path (`\${v//...}`, `\${v/pattern/replacement}`) is affected because `expand_braced_replacement_parameter` has its own value lookup that skips nameref resolution.

---

## 5. Proposed Source-Consistent Fix

### File: `src/executor/expand_braced_replacement.rs`

**Function**: `expand_braced_replacement_parameter`

The fix is to resolve the nameref before the shell-name fallback lookup. This matches GNU's `find_variable()` behavior where the nameref is resolved before the variable value is retrieved.

**Before** (lines 74–81):

```rust
    if is_shell_name(var_name) {
        return Some(
            self.dynamic_parameter_value(var_name)
                .or_else(|| self.env_vars.get(var_name).cloned())
                .map(|value| replace_parameter_pattern(&value, &pattern, &replacement, global))
                .unwrap_or_default(),
        );
    }
```

**After**:

```rust
    if is_shell_name(var_name) {
        let resolved = self.resolved_variable_name(var_name).unwrap_or_else(|| var_name.to_string());
        return Some(
            self.dynamic_parameter_value(var_name)
                .or_else(|| self.env_vars.get(&resolved).cloned())
                .map(|value| replace_parameter_pattern(&value, &pattern, &replacement, global))
                .unwrap_or_default(),
        );
    }
```

This uses `self.resolved_variable_name(var_name)` (from `variable_state.rs` line 31) which returns `Some(target_name)` when `var_name` is a nameref, or `Some(var_name.to_string())` when it's not. The subsequent `env_vars.get(&resolved)` then retrieves the resolved variable's actual value.

**Why this matches C behavior**: GNU's `find_variable()` (variables.c line 2363) calls `find_variable_internal()` and then, if the result has `att_nameref`, calls `find_variable_nameref()` to follow the chain. `resolved_variable_name()` in rubash implements the same logic — it calls `nameref_resolution()` which follows the same chain (up to 16 levels with circular-reference detection), matching `NAMEREF_MAX` in GNU.

**Overlap with other owners**: This fix touches only `expand_braced_replacement.rs`. The nameref resolution infrastructure (`variable_state.rs`) and the declaration output code (`builtins/declare/output.rs`) are not modified. No overlap with `src/lexer/continuation.rs`.

---

## 6. Verification Plan

### Focused Rust test

```rust
#[test]
fn test_nameref_pattern_substitution_resolves_through_chain() {
    // Setup: declare -n v=var, var=abcde
    // Verify: \${v//c/x} == "abxde" (not "var")
}
```

Add to the existing nameref-related tests in `tests/`.

### Official check

```bash
MSYS_NO_PATHCONV=1 wsl bash tests/gnu-compat/run-83.sh check nameref
```

Current state: `DIFF nameref (rubash=932 right=372)` — rubash produces more output than expected because `\${v//...}` returns the nameref target name instead of the resolved value, causing cascading differences.

### Bounded probe verification

```bash
# Verify the minimal reproducer matches GNU
MSYS_NO_PATHCONV=1 wsl bash target/issue-suites/results/probe/nameref-subissue.sh
# Should produce:
# abxde
# abxde
```

```bash
# Verify via rubash
./target/debug/rubash.exe target/issue-suites/results/probe/nameref-subissue.sh
# Should also produce:
# abxde
# abxde
```

---

## Appendix: Assoc Key Ordering (Cosmetic Difference)

### Script (`target/issue-suites/results/probe/assoc-deep.sh`)

```bash
#!/bin/bash
unset wheat
declare -A wheat
wheat=([six]=6 [foo]="bar" [baz]="qux")
declare -p wheat
```

GNU output:
```
declare -A wheat=([foo]="bar" [baz]="qux" [six]="6" )
```

Rubash output:
```
declare -A wheat=([six]="6" [foo]="bar" [baz]="qux" )
```

GNU uses hash-bucket ordering; rubash uses insertion order. This is cosmetic — both contain the same keys and values. The assoc storage in rubash uses `Vec<(String, String)>` for entries (src/builtins/declare/storage/assoc.rs), preserving insertion order. GNU uses a hash table whose traversal order differs. This does not affect correctness, only declare output ordering.

---

## Appendix: Assoc Arithmetic Evaluation

GNU's `declare -Ai chaff=([one]="3+7" [zero]="1+4")` evaluates the arithmetic expressions:
```
declare -Ai chaff=([one]="10" [zero]="5" )
```

Rubash does not evaluate them:
```
declare -Ai chaff=([one]="3+7" [zero]="1+4" )
```

This is because GNU's `declare -i` assignment path evaluates arithmetic expressions during assignment, while rubash stores the raw string. This is a separate issue from the nameref pattern substitution bug and lives in the declare assignment code (`src/builtins/declare/assign.rs`).

---

## Appendix: varenv Test Suite

The `run-83.sh check varenv` shows `DIFF varenv (rubash=1 right=241)` — rubash produces only 1 line for the entire varenv.tests file. This is because varenv.tests relies on sub-scripts (`varenv1.sub` through `varenv25.sub`) and environment setup that don't work correctly in the rubash test harness. Basic `set -k` behavior matches between GNU and rubash in isolated probes. The root cause is likely missing or broken sub-script execution infrastructure, not a specific varenv semantic gap.

---

## Appendix: Array Unset

Basic `unset c[2]` on an indexed array works correctly in both shells:
```bash
c=(a b c d)
unset 'c[2]'
echo "\${c[*]}"  # Both: a b d
```

The array check failures (`DIFF array rubash=142 right=618`) are driven by many other issues (readonly validation, compound assignment, sub-scripts) rather than the specific `unset c[2]` case.
