# GNU/Rubash Semantic Audit: Priority Gaps

This is the broader audit companion to gnu-rust-parameter-brace-audit.md.

## P0

- Heredoc and command substitution: establish canonical owners across parse.y, redir.c, subst.c, lexer/heredoc*.rs, parser command substitution, and executor command substitution. Add bounded tests for quoted delimiters, nested substitution, huge input, unterminated input, and rc/status propagation.
- Coprocess and process substitution: replace bridge-only evidence with external-child materialization tests covering read/write/dup/close/wait and descriptor lifetime.
- File descriptors and redirection: add matrices for ordered redirects, dup/move/close, array elements, nameref targets, <> and /dev/null, invalid and closed descriptors, and heredoc ordering.

## P1

- Parser strictness and status: map parse.y grammar/error propagation to canonical owners and add rc=2 regressions for invalid conditionals, arrays, extglob, arithmetic-for, and substitutions.
- Word expansion, arrays, and arithmetic: model subst.c, array/assoc, and arithmetic owners; cover IFS, quoted arrays, slices, patsub/RHS, and diagnostics.
- Builtins: add canonical rows and focused evidence for complete, getopts, mapfile, read, shopt, trap, type, test, printf, kill, jobs/wait/fg/bg/disown, alias/hash, rsh, and set -r. Keep builtin kill separate from readline kill.
- Lookup, path, glob, and brace: map findcmd/hashcmd/pathexp and the glob/brace/conditional pattern owners; audit escaping, nocase, extglob, and POSIX bracket behavior.
- Jobs, signals, and traps: separate host-owned signal delivery from Rubash trap semantics and cover ERR, DEBUG, RETURN, EXIT, builtin kill, and wait status.

## Structural Drift Risks

- Brace and parameter scanners are duplicated across lexer, parser, and executor.
- Case-pattern and command-substitution separator scanners are duplicated with different right-delimiter rules.
- Glob/extglob matchers are duplicated in storage, executor globbing, and conditional pattern code.
- Existing validators check TSV shape and paths, but not GNU family completeness, owner overlap, selector existence, bridge-free evidence, or scanner duplication.
- Difftest requires per-case timeout, kill-after, stdout/stderr/rc artifact retention, runner identity, and upstream-bridge detection.

## Audit Gates

1. Every active real semantic map row names a concrete Rust owner and executable test selector.
2. Every GNU source family is active, explicitly deferred, host-owned, or documented as unreferenced.
3. New scanners declare their grammar and primary owner; adapters cannot silently duplicate matching logic.
4. Difftest artifacts retain timeout and runner metadata under target/issue-suites/results/.
5. The semantic validator passes both direct shebang invocation and sh scripts/validate-semantic-map.sh.
