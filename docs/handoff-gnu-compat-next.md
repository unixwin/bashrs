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

- Family A (case 37) - unquoted escaped-space loss. `${v-a\ b}` and `${v-c\ d}`: GNU keeps the backslash so each stays one field (`<a b> <x> <c d>`); Rubash drops it and IFS-splits (`<a> <b> <x> <c> <d>`). Isolated and directly comparable; recommended first target.
- Family B (case 38) - single quote consumed in an unquoted alternate word. Unquoted `${IFS+x'a'y}`: GNU `xay`. Double-quoted "${IFS+x'a'y}": GNU `x'a'y`, Rubash `xay`. Inside double quotes the `'` is literal; Rubash treats it as a real quote opener.
- Family D (case 24) - double-quoted alternate word, `'` must stay literal. "${IFS+'$key'}" with `key=value`: GNU `'value'`, Rubash `$key`. Rubash opens a real single quote at the first `'`, which both consumes the quote pair and suppresses `$key` expansion. Same root cause as Family B.
- Family C (cases 2, 3, 8, 9, 11, 12, 28, 29, 33, 34) - quote/brace boundary scanning inside `${IFS+...}`. Examples: GNU `2 ''z}` vs Rubash `2 }z`; GNU `8 "}z` vs Rubash `8 }z`; case 29 brace/quote field drift; cases 33/34 split one quoted word into three. This is the parse.y parameter-scanner depth; the t8 attempt (three quoted alternate return paths in `src/executor/parameter_words.rs`) left `command_chaining::part_063` at 17 passed / 5 failed. Treat as a separate captain-reviewed design task; do not patch parser transitions speculatively.

GNU references: `third_party/bash/parse.y` parameter scanner around 3884-4050 and DOLBRACE transitions 4004-4027; `third_party/bash/subst.c` extraction 1825-2050, `parameter_brace_expand` and `param_expand` around 7663-7780 and 9777-10820.

Harness note: `tests/gnu-compat/run-83.sh` must be driven by a real POSIX shell. `bash` on PATH is the winuxsh (Niubash) shim and exits silently with RC=2 and no output. Verified with both `D:/Git/bin/bash.exe` and `wsl bash -c` as driver - identical results (only the diff header path prefix differs: `/d/repo/rubash` versus `/mnt/d/repo/rubash`). `run_gnu()` always invokes `wsl bash`, so the semantic baseline is WSL GNU regardless of the driver shell.

Next action: Family A first (minimal probe, focused regression, bounded `run-83.sh check posixexp2`), then Families B and D together (shared double-quote-context `'` literal root cause), then Family C as a designed captain-reviewed task.

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
