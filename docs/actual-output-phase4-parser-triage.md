# Phase 4 Parser Triage

Current checkpoint: 8e13124 plus working parameter scanner fix
GNU owner: third_party/bash/parse.y
Rubash owners: src/parser/arithmetic_command.rs, src/parser/subshell_command.rs

## Reproduced Mismatch

Probe:

    ((echo abc; echo def;); echo ghi)
    echo after

GNU output is abc, def, ghi, after with status 0. Rubash previously returned status 1 and no output. Lexer evidence shows separate tokens: (, (, echo, ..., ), ;, echo, ..., ). The first and second parentheses are not an arithmetic command opener because there is no matching adjacent arithmetic close before the command boundary; GNU parses nested subshells.

## Fix

The separated-parenthesis arithmetic parser path now requires a matching double-close token pair. Without that closer it returns control to the subshell parser. The subshell parser applies the same condition so valid arithmetic syntax remains owned by arithmetic_command.rs while nested subshell syntax is accepted.

Permanent regression: separated_double_parentheses_parse_as_nested_subshells.

Validation:

- parser probe: GNU/RUBASH status 0 and identical stdout;
- parser regression: passed;
- arithmetic focused tests: 15/15;
- cargo check: passed;
- git diff --check: passed.


## Parameter Pattern Quote Probe

GNU parse/substitution handling treats the single-quoted pattern in these forms as literal pattern text rather than a command substitution:

    echo parameter-pattern-prefix
    echo parameter-pattern-default
    echo parameter-pattern-alternate

Rubash's pre-parser command-substitution balance checker previously counted the literal dollar-parenthesis inside the parameter pattern and returned status 2 before parsing. The checker now tracks nested parameter depth and pattern quote state. The braced lexer scanner uses the same quote distinction so nested substitutions are not consumed from a single-quoted pattern.

Validation:

- parameter probe reached expansion execution after the fix;
- lexer regression: test_parameter_pattern_quotes_stay_in_one_word;
- lexer tests: 10/10;
- brace fixture status: GNU/RUBASH 0/0 after the fix;
- parser regression: passed;
- git diff --check: passed.

The brace fixture still has separate brace-expansion stdout differences and remains outside this narrow parser/parameter gate. Additional Phase 4 validation passed: parser_tests 350/350, malformed CLI regressions 5/5, and parameter CLI regressions 5/5. A clean multiline POSIX probe now matches GNU: the valid prefix prints a, b, a b, both shells return status 2 for the malformed final expansion, and Rubash emits one syntax diagnostic. The batch precheck now replays only the complete prefix before returning status 2; single-line malformed expansion behavior remains unchanged. The multiline behavior now has permanent coverage in tests/fixtures/malformed_parameter_prefix.sh and malformed_script_preserves_valid_prefix_before_status_two. A clean GNU brace probe based on third_party/bash/braces.c also exposed escaped nested-brace suffix handling: `echo {x,y,\{a,b,c}}` now matches GNU output `x} y} {a} b} c}`. The lexer keeps an adjacent literal `}` in the same word, and command preparation removes shell escapes from the final brace results; coverage is in `expand::braces::tests::test_nested_escaped_brace_preserves_literal_suffix` and `escaped_brace_expansion_preserves_literal_suffix`. Phase 4 remains in progress while the remaining POSIX expansion slices are classified; Phase 5 and later remain locked.

## Clean Valid POSIX Slice

A helper-free probe was added under `target/issue-suites/results/bash-actual/work/posixexp/clean-valid-phase4.sh`. Both shells returned status 0. The remaining stdout differences are now isolated to three forms: quoted assignment expansion preserves `a\ b` in Bash but Rubash emits `a b`; the IFS-plus form containing a quoted literal pattern preserves the single quotes in Bash but Rubash removes them; and the quoted pattern-removal form for `x=foo*bar` emits `foo*bar` in Bash while Rubash retains a trailing `}`. The quoted pattern-removal cases are now fixed by quote-aware `matching_parameter_brace` scanning in `src/executor/parameter_ops.rs`; the focused regression `quoted_parameter_pattern_braces_match_bash` and the existing malformed-parameter regression pass. The quoted assignment escaped-space representation is now fixed by preserving `\ ` during the `=` parameter-assignment side effect; coverage is in `tests/fixtures/parameter_assignment_escaped_space.sh` and `quoted_parameter_assignment_preserves_escaped_space_for_equals`. The remaining clean-slice difference is embedded IFS-plus literal-quote preservation; standalone quoted IFS-plus forms match Bash. The Phase 5 IFS slice now also handles mixed whitespace/non-whitespace separators and passes `unquoted_at_preserves_ifs_boundary_fields`; checkpoint `c6225ec`. The stale suite's `recho` diagnostics are not used as evidence.
