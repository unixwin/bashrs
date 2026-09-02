# Handoff Prompt — posixexp2 expansion-stage Interp 221 (task #3)

Paste everything below the line into a fresh session.

---

You are continuing GNU Bash 5.2 compatibility work on rubash (Rust shell,
`D:\repo\rubash`, branch `fresh-master`). Read `agents.md` first. Hard rules:

- Verification baseline is ONLY WSL GNU Bash 5.2.21 via script FILES.
  Never `wsl bash -c "..."` for backslash-heavy cases (passthrough mangles `\\`).
- A PASS claim for posixexp2 is valid ONLY if
  `MSYS_NO_PATHCONV=1 bash tests/gnu-compat/run-83.sh check posixexp2`
  prints `PASS posixexp2`.
- Never run `third_party/bash/tests/posixexp2.tests` directly in rubash:
  `execute_upstream_handler_script` (handlers_b.rs:110) fires on that path and
  prints CANNED output (a fake 100% match). Always use the harness copy:
  `bash tests/gnu-compat/run-83.sh prepare` then
  `target/upstream-tests/posixexp2.tests`.
- Do not commit. The captain commits after verification.
- `src/lexer/continuation.rs` is captain-exclusive.
- Keep `cargo build` clean. The only acceptable warning is the pre-existing
  `unused variable: env_vars` in src/builtins/setattr/value.rs:31.

## Goal

Make posixexp2 pass with REAL semantics. GNU parse.y's dolbrace state machine
(Austin Group Interp 221): in POSIX mode, inside DOUBLE QUOTES, a single quote
inside `${...}` is a LITERAL character, so the first unquoted `}` closes the
expansion. Non-posix or unquoted: `'` opens a nested single-quote.

## Already done (compiled, verified)

1. Parse-time posix tracking (Layer A): the lexer tracks `set -o posix`
   per logical line (`tokenize_with_initial_posix` in src/lexer/mod.rs);
   `src/main.rs` seeds it from `__RUBASH_POSIX_MODE`; 5 command-substitution
   call sites pass `self.posix_mode_enabled()`. The whole file now parses and
   runs to completion (it used to die at line 20).
2. Step 1: src/executor/parameter_ops.rs:235
   `matching_parameter_brace_in_context(input, outer_double_quote, posix)` —
   feeds `BraceContext` into `crate::lexer::dolbrace::scan_braced_parameter_body`
   (which already implements Interp 221 at dolbrace.rs:121-138; its `end` is the
   byte offset just past the closing `}` in the body-after-`${` string), with a
   fallback loop after it. `matching_parameter_brace` is now a `(false,false)`
   wrapper. `braced_parameter_spans_whole_word` at parameter_ops.rs:322 still
   uses the non-posix wrapper.

## Current failing cases (diff vs tests/gnu-compat/upstream-rights/posixexp2.right)

Reproduce: `cargo build && bash tests/gnu-compat/run-83.sh prepare &&
target/debug/rubash.exe target/upstream-tests/posixexp2.tests | diff - tests/gnu-compat/upstream-rights/posixexp2.right`

- 2 `"${IFS+'}'z}"` → GNU `''z}`, we print `'}'z`
- 3 `"foo ${IFS+'bar} baz"` → GNU `foo 'bar baz`, we print `foo 'bar} baz`
- 8 `"${IFS+\"}\"z}"` → GNU `""z}`, we print `\"z}`
- 9 `"${IFS+\"\}}"z}"` → GNU `"}"z`, we print `\\}"z`
- 11 `"$(echo "${IFS+'}'z}")"` → GNU `''z}`, we emit syntax error `unexpected EOF while looking for matching ')'` + `failed in 11`
- 12 `"$(echo ${IFS+'}'z})"` → GNU `}z`, we print `z}`
- 14 `"${IFS+\}z}"` → GNU `}z`, we print `\}z`
- 15 compound nesting → GNU `<foo abx{ {{{}b c d{} bar> ...`, we print `{{{\}b`
- 27 `${IFS+"'$key'"}` (unquoted) → GNU `'value'`, we print `$key`
- 28, 29 → quoting/splitting divergences (see .right)
- 37 `${v-a\ b}` (v unset) → GNU `<a b> <x> <c d>`, we print `<a> <b> <x> <a> <b>`
- 39 `"${foo%*'a'*}"` → GNU `x'`, we print `x`

Cases 2/3/11/12 are the core Interp 221 pairing cases; the plan below fixes
them. 8/9/14/15/27-29/37/39 are separate sub-rules — probe each one against
WSL with a standalone script containing `set -o posix` before fixing.

## Architecture findings (verified this session)

1. src/executor/embedded_mutations.rs, `expand_embedded_parameters_ordered_mut`,
   the `Some('{')` arm (~line 181): after
   `expand_current_shell_braced_substitution` (only fires for `${|` /
   `${<whitespace>`, defined line 309) it calls
   `collect_braced_parameter_name` (src/executor/command_subst_helpers.rs:3),
   which is NOT posix-aware. This is the Step 2 seam. The word arrives with
   outer quotes stripped; `context` is `SubstitutionQuoteContext::DoubleQuoted`
   for `"..."` words.
2. src/executor/parameter_core.rs:147-158 routes whole-word `${...}` to
   `expand_quoted_parameter_word_mut` gated on
   `braced_parameter_spans_whole_word` (non-posix) — so `${IFS+'}'z}` currently
   passes the gate and enters the wrong operator arm. This is Step 3 seam B.
   Words may also reach `expand_quoted_parameter_word_mut` directly via the
   `\x1d` marker path (parameter_core.rs:40-44); verify empirically which path
   case 2 takes (RUBASH_DBG_TILDE=1 prints at parameter_core.rs:153).
3. src/executor/parameter_words.rs:182 `expand_quoted_parameter_word_mut`:
   operator arms. An alternate word of `'` survives
   `expand_embedded_parameters_mut_with_context(_, DoubleQuoted)` unchanged, so
   the arms are fine textually once pairing is fixed.

## Exact planned edits

A. src/executor/parameter_ops.rs — add:

```rust
pub(in crate::executor) fn braced_parameter_spans_whole_word_in_context(
    word: &str,
    outer_double_quote: bool,
    posix: bool,
) -> bool {
    let Some(rest) = word.strip_prefix("${") else {
        return false;
    };
    matching_parameter_brace_in_context(rest, outer_double_quote, posix)
        .is_some_and(|index| index + 1 == rest.len())
}
```

B. src/executor/parameter_core.rs:147-158 — gate on context-aware spans:

```rust
let posix_dquote = matches!(context, SubstitutionQuoteContext::DoubleQuoted)
    && self.posix_mode_enabled();
let spans = if posix_dquote {
    braced_parameter_spans_whole_word_in_context(word, true, true)
} else {
    braced_parameter_spans_whole_word(word)
};
if spans { return self.expand_quoted_parameter_word_mut(word, context); }
```

So `${IFS+'}'z}` (closes at 7 of 11) falls through to
`expand_embedded_parameters_mut_with_context`.

C. src/executor/embedded_mutations.rs `Some('{')` arm — before falling back to
`collect_braced_parameter_name`:

```rust
if matches!(context, SubstitutionQuoteContext::DoubleQuoted)
    && self.posix_mode_enabled()
{
    let remainder = chars.as_str();
    if let Some(close) = matching_parameter_brace_in_context(remainder, true, true) {
        let consumed = word.len() - remainder.len();
        let name = remainder[..close].to_string();
        *chars = word[consumed + close + 1..].chars().peekable();
        // then expand format!("${{{name}}}") with `context` inside
        // expand_with_parameter_env exactly like the existing fallback
    }
}
```

(`chars` is a local `Peekable<Chars>`; reassignment is safe. `close` is a char
boundary. If the matcher returns None, fall back to the old collector.)

D. src/executor/parameter_words.rs:182 top of `expand_quoted_parameter_word_mut`
(head/tail split, needed if a word arrives via the `\x1d` direct path):

```rust
if matches!(context, SubstitutionQuoteContext::DoubleQuoted)
    && self.posix_mode_enabled()
{
    if let Some(rest) = word.strip_prefix("${") {
        if let Some(close) = matching_parameter_brace_in_context(rest, true, true) {
            if close + 1 < rest.len() {
                let braced_end = 2 + close + 1;
                let head = self.expand_quoted_parameter_word_mut(&word[..braced_end], context);
                let tail = self.expand_embedded_parameters_mut_with_context(&word[braced_end..], context);
                return format!("{head}{tail}");
            }
        }
    }
}
```

Head recursion is safe because edit A makes `${IFS+'}` pass the spans gate
under the posix-aware matcher (`'` literal, `}` closes), so it reaches the `+`
arm and returns `'`.

Expected trace for case 2: word `${IFS+'}'z}` → spans(B)=false → embedded
collector (C) name=`IFS+'`, remainder `'z}` → head `${IFS+'}` → `+` arm → `'` →
total `''z}`. Case 3: `foo ${IFS+'bar} baz` → name `IFS+'bar` → `foo 'bar baz`.

## Verification steps

1. `cargo build` clean.
2. Probe: `target/rb-probe/c23.sh` (contains `set -o posix` + cases 2/3).
   WSL: `MSYS_NO_PATHCONV=1 wsl bash /mnt/d/repo/rubash/target/rb-probe/c23.sh`
   must print `2 ''z}` / `3 foo 'bar baz`; rubash must match.
3. Full diff vs posixexp2.right (command above).
4. Iterate per-case for 8/9/14/15/27-29/37/39 with WSL probes; consult
   third_party/bash parse.y / subst.c when stuck.
5. Official: `bash tests/gnu-compat/run-83.sh check posixexp2` → `PASS posixexp2`.

## After posixexp2

Task #4: ifs-posix (function-call assignment breaks for-loop iteration var;
RUN83_TIMEOUT=180). Task #5: split cond into parser + regex tracks. Task #2:
docs pinning — ledger at target/issue-suites/results/check-run-20260831-pinned.log
(PASS=11/DIFF=64/TIMEOUT=5/SKIP=3); update docs/COMPATIBILITY-STATUS.md,
docs/DIFF-MASTER-PLAN.md, docs/issue-suite-diff-analysis.md honestly.
