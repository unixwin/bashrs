# Typed Expansion Migration Checkpoint

Updated: 2026-08-23
Branch: agentteams/typed-provenance

## Purpose

This is the durable recursive checkpoint for GNU Bash substitution and expansion work. The primary GNU owners are third_party/bash/subst.c (read_comsub, command_substitute, param_expand, expand_word_internal, shell_expand_word_list) and third_party/bash/redir.c (here-string, heredoc, input redirection, dynamic fd ownership).

The migration chain is:

lexer/parser -> raw word metadata -> parameter scanner -> typed substitution carrier -> quote/splitting context -> assignment/path/redirect boundary -> command execution -> status/trap/state restoration -> materialization

A fix is incomplete when it only changes the final assertion and leaves the upstream carrier or semantic owner ambiguous.

## Typed Carriers

Current carriers are SubstitutionOutput { bytes, status, context }, ExpandedFragment { bytes, quoted, splittable }, and ExpandedWord { fragments, status }. SubstitutionOutput readback removes NUL and trailing LF according to command-substitution rules, while preserving other bytes until an explicit text boundary.

## Completed Pushed Commits

- ec94603d typed mutable word expansion boundary
- 278652ed here-string mutable expansion
- 082c96d2 independent mutable heredoc boundary
- 8f432190 loop-fd heredoc owner
- 1eea003e read heredoc owner
- 2c5f9a8e mapfile heredoc owner
- ad13029d external fd heredoc owner
- dff6879d external stdin mutable wrapper
- d267f422 pipeline and trap stdin owners
- bf33127f function stdin owner
- d91b972c read timeout stdin owner
- 0a357ffd external file and read stdin owners
- e339e716 external sed stdin owner
- cd00e4a7 typed heredoc substitution boundary
- bb4f4639 direct mutable heredoc shortcut input
- 6d2f3602 Windows extended path normalization for cd ..; pwd

## Current Owner Graph

Migrated mutable owners: command_input_scope, compound_exec loop heredocs, read_io, mapfile_helpers, external_setup, external_inner, external_file_builtins, function_calls, pipeline_exec, trap_exec, and embedded_mutations typed command substitution.

Intentional legacy boundaries: shell_options stdin_string_for_command(&self) for immutable utility and command-substitution callers; command_substitution_heredoc_output(&self) for the immutable path; decode_command_substitution_payload in assignment, heredoc legacy, backtick, and command-preparation boundaries. These cannot be removed by replacing them with lossy UTF-8 conversion.

## Focused Evidence

Repeated passing gates:

- cargo check
- cargo test --test executor_tests command_chaining::part_080: 156 passed
- cargo test --test heredoc_regressions: 2 passed
- cargo test --test cli_tests read: 30 passed
- cargo test --test cli_tests mapfile: 3 passed
- command_chaining part_005 heredoc tests: passing

Known unrelated command-substitution slice failure: bashdb_info_files_reports_source_files_without_command_substitution_error, a fixed bashdb path assertion.

## Persistent Root-Cause Failures

1. test_current_shell_command_substitution_captures_stdout_and_keeps_side_effects reports bad substitution for the current-shell form: ${ value=new; echo alpha; echo; }.
2. test_current_shell_reply_substitution_expands_inside_command_substitutions leaves command text such as combined comsubs; and comsubs; });.
3. An experiment that called expand_embedded_parameters_mut_with_context before assignment preprocessing did not fix either test and was reverted.
4. A command-substitution dispatch reorder was tested and reverted because the focused slice did not improve.

## Recursive Next Steps

1. Add scanner tests for current-shell braced forms, nested braces, quotes, escaped braces, and ${| command } reply mode.
2. Replace the boolean word_contains_current_shell_command_substitution detector with a structured span/source result.
3. Route that span through expand_command_substitution_mut_typed_with_context, preserving side effects, REPLY, status, positional parameters, and environment restoration.
4. Then migrate assignment and command-preparation marker decoding to typed fragments.
5. Add differential tests for raw C0, NUL, repeated LF, invalid UTF-8, arithmetic status 1 versus 127, printf bytes, arrays, eval, heredocs, traps, and redirects.
6. Update docs/semantic-ownership.tsv only after focused evidence and validate with scripts/validate-semantic-map.sh.

## Recursion Rules

Read this checkpoint before changing the next owner. Do not modify third_party/bash. Do not run unbounded full suites. Keep artifacts under target/issue-suites/results. Run git diff --check, cargo check, and the smallest relevant focused tests before each commit. Check for residual rubash.exe, bash.exe, cargo.exe, and suite runners before closing a testing turn. Never claim completion while a persistent failure or intentional legacy boundary remains.
