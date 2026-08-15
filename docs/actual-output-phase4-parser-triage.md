# Phase 4 Parser Triage

Current checkpoint: 9d82f0d plus working parser fix
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

Phase 4 remains in progress. The full native arithmetic fixture still has a separate later status/diagnostic interaction after this parser point; it must be isolated before the parser phase gate closes. Phase 5 and later remain locked.
