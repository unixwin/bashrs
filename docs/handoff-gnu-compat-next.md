# GNU Bash Compatibility Handoff

Repository: D:/repo/rubash
Branch: fresh-master
Scope: strict GNU Bash source-driven compatibility repair

## Verification Rules

- Semantic baseline is WSL GNU Bash 5.2.21 via script files.
- Use MSYS_NO_PATHCONV=1 wsl bash /mnt/d/repo/rubash/<script>.
- Do not use the Winuxsh bash shim or Git Bash as a semantic baseline.
- Do not use wsl bash -c for backslash-heavy or nested-quote probes.
- A full-suite PASS claim requires the current bounded run-83.sh check result.
- Never modify expected output to manufacture parity.
- Before a semantic edit, record the GNU source function/range, Rust owner, and minimal probe.
- Native lldb.exe is the Rust debugger for the MSVC toolchain. Capture useful sessions under target/issue-suites/results/ and remove temporary source traces.
- src/lexer/continuation.rs is captain-exclusive.
- Every shared-tree edit must leave cargo build passing.

## Accepted Repairs

### mapfile CRLF and NUL delimiter

Files:

- src/executor/mapfile_helpers.rs
- src/executor/arrays/mapfile.rs
- tests/executor_command_chaining/part_011.rs

Root cause: ordinary fd-0 file redirects used fs::read_to_string, dropping CR from GNU CRLF mapfile.data. The NUL delimiter path also needed to consume NUL as GNU mapfile -d '' does.

Evidence:

- GNU mapfile.data begins with CRLF bytes and GNU retains CR.
- Focused CRLF regression: 1/1 passed.
- Official WSL check: PASS mapfile, PASS=1 DIFF=0 TIMEOUT=0 SKIP=0.
- cargo build passed with only the pre-existing setattr/value.rs unused env_vars warning.

Do not reopen this family unless a new minimal GNU mismatch is produced.

### IFS positional state synchronization

Files:

- src/executor/alias_set_builtins.rs
- src/executor/embedded_mutations.rs
- src/executor/external_finish.rs
- src/executor/function_calls.rs
- src/executor/shell_options.rs
- src/executor/shift_echo_builtins.rs
- tests/executor_command_chaining/part_080.rs

Root cause: compound pipeline stages copied stale shell_state.positional. Some set, shift, function entry/restore, command-substitution save/restore, alias set --, and external setup/restore paths updated only Executor.positional_params. The stale clone caused duplicate function-loop execution when function-local set/shift was followed by a pipeline/subshell.

GNU source: third_party/bash/execute_cmd.c, execute_for_command/list snapshot around lines 2990-3144, especially list expansion near 3017 and iteration near 3035.
Rust state-copy evidence: src/executor/command_substitution.rs around 472-485.
Probe: target/ifs-min.sh, with function-local set x $i; shift; echo "$i" | (IFS=$ifs; read x y; printf ...).

Evidence:

- WSL GNU and Rubash both emit 2 iterations for target/ifs-min.sh.
- focused regression test_ifs_set_shift_pipeline_preserves_for_iteration_state: 1/1 passed.
- cargo build --bin rubash passed.
- Full ifs-posix remains 3850/6856, with 3006 broader failures. Do not attribute those remaining failures to this repair.
- The only intentional direct positional assignment is inside public_accessors::set_positional_params; callers route through that synchronization entry point.

## Focused-Only Repairs, Not Full-Suite Acceptance

### cond quote-marker boundary

File: src/parser/conditional_command.rs

GNU basis: third_party/bash/parse.y cond_expr around 5010-5248; probes in third_party/bash/tests/cond.tests around 68-106 and 232-241.

Change: public conditional argument values strip private \x11 quote markers while raw metadata remains available for pattern semantics.

Evidence: cargo test --test parser_tests conditional_tests = 16/16; malformed_conditional = 1/1; cargo build passes.
Official cond check is not valid while target/upstream-tests subfixtures are missing and live invokes nested WSL. Do not claim full cond parity until a valid fixture-backed check exists.

### brace invalid nested sequences

File: src/expand/braces.rs

GNU basis: third_party/bash/braces.c brace_expand around 121-159; invalid BRACE_SEQ candidates are skipped around 136-141. expand_seqterm around 242-263 preserves a failed sequence when trailing text remains.

Change: invalid outer sequences no longer trigger iterative re-expansion; nested comma groups can expand while nested sequence groups remain literal.

Evidence: brace focused tests 14/14; cargo build passes; focused GNU/Rubash probes agree.
The official braces result 132/77 is contaminated by missing zecho and stale GNU errors; it is not an acceptance result.

## Open Work

### posixexp2: quote preservation in ${IFS+word} alternate words (refreshed 2026-09-05)

Current authoritative artifacts:

- target/issue-suites/results/check/posixexp2.rubash.out
- tests/gnu-compat/upstream-rights/posixexp2.right
- target/issue-suites/results/check/posixexp2.diff
- target/issue-suites/results/live/posixexp2.gnu.out and .rubash.out

Baseline validity: `tests/gnu-compat/upstream-rights/posixexp2.right` is byte-identical to the live WSL GNU Bash 5.2.21 output (`diff` empty; both 40 lines), so the .right file is a valid authority for this family.

Refreshed difference set. The previous list (cases 8, 9, 11, 12, 28, 29, 37, 39) is stale. Rubash now differs in exactly 13 of 40 cases: 2, 3, 8, 9, 11, 12, 24, 28, 29, 33, 34, 37, 38. Case 39 now matches.

All 40 cases exercise the quote semantics of the alternate/default word in `${IFS+word}` and `${var-word}` under `set -o posix; shopt -u xpg_echo`. Minimal probe kept at `target/px2probe.sh`, compared as `MSYS_NO_PATHCONV=1 wsl bash /mnt/d/repo/rubash/target/px2probe.sh` versus `target/debug/rubash.exe target/px2probe.sh`.

Observed root-cause families:

- Family A (case 37) - FIXED in `39403609`. Unquoted escaped-space loss: `${v-a\ b}` and `${v-c\ d}` produced two fields in Rubash (`<a> <b> <x> <c> <d>`) where GNU keeps one each (`<a b> <x> <c d>`). Root cause: `src/executor/command_prepare.rs::braced_alternate_word_values` routed the narrow `-`/`:-` unquoted alternate through the String operator path, whose `unescape_remaining_shell_escapes` converted `\ ` into a real separator before field splitting ran. Such alternates now route through the quote-aware fragment expansion when they contain a backslash-escaped IFS whitespace (`parameter_word_has_escaped_whitespace` in `src/executor/parameter_ops.rs`); non-whitespace escapes such as `${v-foo\\bar}` stay on the String path and still match GNU (`<foo\\bar>`). Regression: `unquoted_parameter_default_preserves_escaped_space_field_boundary`. Bounded `run-83.sh check posixexp` output is byte-identical to HEAD across a clean-build A/B, so the re-routing changed nothing outside the escaped-whitespace case.
- Family B (case 38) - single quote consumed in an unquoted alternate word. Unquoted `${IFS+x'a'y}`: GNU `xay`. Double-quoted "${IFS+x'a'y}": GNU `x'a'y`, Rubash `xay`. Inside double quotes the `'` is literal; Rubash was treating it as a real quote opener because the lexer double-quote marker was dropped before expansion. FIXED in this pass; see the section below for the actual root cause.
- Family D (case 24) - double-quoted alternate word, `'` must stay literal. "${IFS+'$key'}" with `key=value`: GNU `'value'`, Rubash `$key`. Rubash opens a real single quote at the first `'`, which both consumes the quote pair and suppresses `$key` expansion. Same root cause as Family B. FIXED in this pass.
- Family C (cases 8, 9, 11, 12, 28, 29) - REMAINING. Quote/brace boundary scanning inside `${IFS+...}`. Cases 2, 3, 33, 34 from the original list were fixed by the double-quote-marker fix below. Examples: GNU `2 ''z}` vs Rubash `2 }z`; GNU `8 "}z` vs Rubash `8 }z`; case 29 brace/quote field drift; cases 33/34 split one quoted word into three. This is the parse.y parameter-scanner depth; the t8 attempt (three quoted alternate return paths in `src/executor/parameter_words.rs`) left `command_chaining::part_063` at 17 passed / 5 failed. Treat as a separate captain-reviewed design task; do not patch parser transitions speculatively.

GNU references: `third_party/bash/parse.y` parameter scanner around 3884-4050 and DOLBRACE transitions 4004-4027; `third_party/bash/subst.c` extraction 1825-2050, `parameter_brace_expand` and `param_expand` around 7663-7780 and 9777-10820.

Harness note: `tests/gnu-compat/run-83.sh` must be driven by a real POSIX shell. `bash` on PATH is the winuxsh (Niubash) shim and exits silently with RC=2 and no output. Verified with both `D:/Git/bin/bash.exe` and `wsl bash -c` as driver - identical results (only the diff header path prefix differs: `/d/repo/rubash` versus `/mnt/d/repo/rubash`). `run_gnu()` always invokes `wsl bash`, so the semantic baseline is WSL GNU regardless of the driver shell.

Next action: the remaining Family C cases (8, 9, 11, 12, 28, 29) as a designed captain-reviewed task (parse.y parameter-scanner depth).

### posixexp2 Families B/D: FIXED (2026-09-05)

Families B and D are fixed, together with four Family C cases (2, 3, 33, 34). `run-83.sh check posixexp2` now differs from the `.right` on 6 of 40 cases (8, 9, 11, 12, 28, 29) instead of 12.

An earlier pass concluded the gap was upstream of the executor and marked the family blocked on a WORD_DESC quote-state change. That conclusion was wrong, and the instrumentation behind it was misread: `{:?}` Debug formatting on a `&str` adds its own surrounding double quotes, so a log line `ECW["${IFS+'$key'}"]` was read as "the word contains literal quotes" when it actually reported the bare `${IFS+'$key'}`. Re-instrumenting with an explicit `has1d = word.starts_with('\x1d')` flag showed the sentinel was present all along.

Real root cause, in `src/executor/command_prepare.rs::expand_command_word`:

1. The brace-expansion block ran on words carrying the `\x1d` double-quote marker. `expand_braces_with_optional_raw` rebuilds its results from `raw`, which never carries the marker, so it returned a marker-less word.
2. The single-element-but-changed branch then recursed with `raw = None`, so the recursion lost double-quote context entirely.
3. With no marker, `braced_alternate_word_values` took over and re-lexed the alternate through `expand_alternate_word_fragment` as an independent command, where the inner single quotes became real quotes and `$key` was protected from expansion.

Fix (two conditions in the same function):

- Skip brace expansion when the word starts with the `\x1d` marker - a fully double-quoted word never brace-expands in GNU anyway, and the marker would otherwise be dropped by the raw-based rebuild. Double-quoted `$@`/`$*` words are excluded: those reach the per-positional split through the raw-based path, and skipping it collapsed `"${1+  $@  }"` to one word (exp suite).
- Treat the marker as authoritative for the substitution quote context: a word carrying it gets `SubstitutionQuoteContext::DoubleQuoted`, so `expand_embedded_parameters_mut_with_context` keeps inner `'` literal and still expands `$key`. The previous `scan_substitution_spans`-derived context reported Unquoted for these whole-word braced forms.

Regression: `compat_issue_regressions::double_quoted_parameter_alternate_keeps_quotes_literal`, covering `"${IFS+'$key'}"`, `"${IFS+x'a'y}"`, `"${IFS+'quoted word'}"`, `"${v-'a b'}"`, and the control `"${v-foo\bar}"`. Minimal GNU-vs-Rubash probe kept at `target/bdq.sh` (10 cases, byte-identical).

Suites: `cargo test --lib` 10 failed (unchanged), `--test parser_tests` 13 failed (unchanged), `--test heredoc_regressions` 2 passed, `--test cli_tests` 344 passed / 47 failed against a 336 passed / 54 failed baseline - 7 existing cli_tests repaired, zero new failures, plus the new regression. A/B over the six upstream suites with output change, scored as multiset distance to the `.right` (lower is better): exp 142 -> 139, assoc 427 -> 419, braces 23 -> 18, more-exp 263 -> 259, nameref 375 -> 364, new-exp 297 -> 295. All six improved; `tilde2` moved DIFF -> PASS in the full 83-suite check with no suite regressing PASS -> DIFF.

### Subshell variable isolation - FIXED

While fixing Family A, bisecting the posixexp2 case-37 prefix located a separate
defect at case 36, which assigns `v` inside a subshell and then reads it from the
parent. WSL GNU Bash 5.2.21 keeps that assignment local (V value is empty), so
case 37 falls back to its default word. Rubash leaked it, so case 37 expanded
`v`'s stored value and then IFS-split it.

Root cause: `src/executor/compound_exec.rs::execute_subshell_command_with_redirects`
runs the subshell body in place on the parent executor and restored only
`env_vars`, `pipestatus`, `subshell_depth`, and `loop_depth`. The typed variable
store `shell_state.variables` and `positional_params` were never saved, so
`(v=x)`, `(set -- ...)`, and `(IFS=...)` all leaked into the parent.
`src/executor/command_substitution.rs::command_substitution_executor` was already
correct because it clones the whole executor into a throwaway instance. `unset`
was already isolated because it removes from `env_vars`, which the existing
restore covers. Background jobs are unaffected: `execute_background_ast_command`
spawns a real child `rubash -c` process, so isolation there was already at the
process boundary. The other three `saved_env` sites (`external_finish.rs` and
`embedded_mutations.rs` x2) were checked and are correct - the first restores the
whole `shell_state`, the latter two wrap function calls whose variables are
isolated by `function_locals.rs`.

Fix: save `shell_state.variables` and `positional_params` at the subshell
boundary and restore both on the success and error paths.

Evidence: `target/subshell_iso.sh`, `target/subiso2.sh`, `target/subiso3.sh`,
`target/subiso4.sh`, and `target/assign36.sh` now match WSL GNU Bash 5.2.21 on
stdout and exit status. Regression:
`subshell_keeps_variable_assignment_and_positional_params_local`. posixexp2
drops from 13 to 12 differing cases (case 37 gone).

A/B against a clean HEAD build over all 83 suites: summary unchanged
(PASS=15 DIFF=63 TIMEOUT=2 SKIP=3); 20 suites changed output, 61 identical.
Measuring lines of `.right` absent from Rubash output, the fix lands 28 lines
closer overall (set-x +10, arith-for +7, arith +3, redir +2, test +2, varenv +2,
vredir +2, exp +1, history +1, comsub-posix +1, posixexp2 +1) with func and
builtins each 2 lines further. Those two are merge-ordering artifacts only:
captured with separate stdout and stderr, the diagnostic is intact on stderr
and `11111 ()` is intact on stdout, so there is no semantic regression.

### comsub-posix: collector/heredoc boundary

Valid live artifacts:

- target/issue-suites/results/wsl-live/comsub-posix.gnu.out
- target/issue-suites/results/wsl-live/comsub-posix.rubash.out

Rubash stops near comsub-posix.tests line 64 with unexpected EOF and may execute heredoc terminator as a command. Minimal reproducer:

    echo $(cat <<eof
    here doc with )
    eof
    )

Quoted heredoc alone passes. Root cause: tokenize_with_heredocs accumulates an open $(...) through the heredoc body; when ) closes the substitution, the line iterator is exhausted and the heredoc collector has no body. A gate bypass worsened the behavior and was reverted.

GNU owners: third_party/bash/subst.c command_substitute and third_party/bash/parse.y heredoc/command-substitution grammar.
Rust owners: src/lexer/mod.rs, src/lexer/heredoc.rs, src/executor/embedded_mutations.rs collect_command_substitution_source around 693-823.

No verified fix or regression exists. Implement tokenizer/heredoc state coordination; do not simply bypass the continuation gate and do not edit continuation.rs without captain approval.

### Remaining IFS audit

The proven t10/t13 repair is accepted above. Any new IFS work must start from a new task and a new minimal reproducer. The prior full result was 3850/6856 and includes broader failures. Audit direct positional writes only with GNU variables.c/execute_cmd.c evidence.

## Harness and Artifact Warnings

- bash-actual-current.log PASS for posixexp2 is stale and must not be used.
- A check diff can have equal output line counts while still containing content differences.
- Some official checks are contaminated by missing recho/zecho helpers, missing target/upstream-tests fixtures, stale GNU errors, or nested WSL invocation. Record the blocker and do not certify parity.
- The known pre-existing build warning is src/builtins/setattr/value.rs:31 unused env_vars.
- Check for stuck rubash.exe, bash.exe, cargo.exe, and suite processes after long test turns.

## Suggested Next Sequence

1. Verify the current shared tree with cargo build.
2. Finish posixexp2 with fresh minimal probes for cases 8/9/11/12/28/29/37/39.
3. Finish comsub heredoc collection with a source-backed focused regression.
4. Re-run valid mapfile, braces, cond, posixexp2, comsub-posix, and ifs probes.
5. Update docs/COMPATIBILITY-STATUS.md only with fresh evidence.
6. Review git diff and separate accepted product changes from unrelated pre-existing work before committing.
