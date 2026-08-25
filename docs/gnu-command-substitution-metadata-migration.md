# GNU Command Substitution Metadata Migration

Status: in progress
Owner: Rubash executor/parser compatibility work

## Scope

This document records the Rubash migration from global C0 sentinel cleanup toward the GNU Bash model in third_party/parse.y and third_party/subst.c. The active slice is command substitution, quoted word expansion, IFS splitting, heredoc expansion, assignment storage, and final quote removal. Bashdb is an integration witness; bashdb product files remain external and unchanged.

## GNU Contract

GNU keeps lexical quote state separate from payload bytes:

- parse.y builds WORD_DESC data while tracking quoted/pass-next state and protects literal CTLESC/CTLNUL.
- subst.h carries Q_DOUBLE_QUOTES and Q_HERE_DOCUMENT independently from the string.
- subst.c::command_substitute executes the complete substitution in a child with fd 1 connected to a pipe.
- subst.c::read_comsub owns capture/readback: it removes NUL bytes, applies context-sensitive protection, strips trailing newlines, and returns the word plus flags.
- expand_word_internal carries quoted-null and split policy until word_list_split.
- string_quote_removal is paired and scoped; it is not global marker deletion.

## Rubash Owners

| GNU contract | Rubash owner | Current state |
|---|---|---|
| command_substitute/read_comsub | executor/command_substitution.rs; executor/substitution_metadata.rs | partial |
| lexical word metadata | parser/nodes.rs; lexer/word.rs; lexer/quotes.rs | partial, sentinel-backed |
| embedded substitution expansion | executor/embedded_parameters.rs; executor/embedded_mutations.rs | partial |
| final word materialization | executor/command_prepare.rs; executor/expand_word.rs | partial, heuristic-backed |
| IFS splitting | executor/command_prepare.rs | partial |
| heredoc expansion | executor/command_input_scope.rs; parser/heredoc.rs | partial |

## Architecture

The target data flow is:

    parser lexical context
      -> WordPart / QuoteMeta sidecar
      -> SubstitutionOutput { bytes, status, context, split_policy }
      -> ExpandedWord fragments
      -> one IFS-aware splitter
      -> one final quote-removal/glob boundary

Payload bytes must remain bytes. Syntax quotes from the outer source are metadata; quote bytes printed by the child are data. No new global sentinel is allowed. Transitional string codecs, if required, must be owner-tagged, length-delimited, self-escaping, and decoded exactly once by their owner.

## Checklist

- [x] Add an owner-scoped SubstitutionOutput readback type.
- [x] Route parsed substitution capture through typed readback for NUL and trailing newline handling.
- [x] Add unit coverage proving child quote bytes remain data.
- [~] Carry lexical context into parsed readback. Single-span raw words now select context before mutable word expansion, and the context reaches embedded expansion/AST fallback; multi-span materialization and heredoc/assignment context remain open.
- [ ] Represent quoted, unquoted, assignment, and heredoc substitution context without C0 prefixes.
- [~] Add ExpandedWord/WordPart fragments with origin, quoted state, escaped state, quoted-null state, expansion state, and no-split policy. Simple multi-span mixed words now materialize RawWordFragment values into ExpandedFragment and use the typed splitter; parameter/arithmetic/heredoc combinations remain open.
- [~] Replace command_substitution_word_split and raw-word heuristics with centralized IFS-aware splitting. The typed splitter is implemented and unit-tested, but whole-word integration was reverted after it changed bashdb getopts/profile behavior.
- [~] Preserve quoted empty fields and adjacent quoted/unquoted fragments. Typed origin-aware splitting now keeps literal IFS bytes intact, and focused coverage confirms quoted empty substitutions remain one empty word while unquoted empty substitutions disappear; broader quoted-null combinations remain open.
- [ ] Move final quote removal and glob protection to one owning boundary.
- [ ] Remove targeted early substitution restoration from expand_word, command_substitution_values, command_input_scope, and backtick paths as each owner migrates.
- [ ] Replace \x1c structural close detection with parser state/typed heredoc termination.
- [ ] Add byte probes for payload values 0x10..0x1f, quotes, backslashes, nested substitutions, and heredocs.
- [ ] Add GNU/Rubash differential tests for IFS, quoted-null, assignments, and nested substitutions.
- [ ] Re-run bashdb nested-shell compatibility and classify Pygments/host noise separately.
- [ ] Update this document with commit ids, test commands, and remaining risks.

## Focused Differential Matrix

Each probe compares GNU D:/Git/bin/bash.exe with target/debug/rubash.exe and checks argc and bytes:

1. $(printf '"x y"') versus "$(printf '"x y"')".
2. x=$(printf '%s\n' '"x"'); declare -p x.
3. IFS=:; set -- $(printf 'a::b:'); printf '<%s>\n' "$@".
4. set -- "$(printf '')" and set -- $(printf '').
5. set -- pre$(printf 'a b')post and quoted equivalent.
6. Nested substitution with child-emitted quotes and backslashes.
7. Assignment and heredoc substitutions.
8. Payload bytes 0x10..0x1f with no marker leakage.

## Evidence Log

- 6fe14287: added SubstitutionOutput and typed parsed readback.
- 3dc2cfab: documented the migration, threaded SubstitutionQuoteContext through parsed readback, and added ExpandedFragment/IFS splitter tests.
- Current follow-up: parsed readback accepts lexical context, complete double-quoted parent handoff is wired, and both AST and mutable embedded-substitution fallbacks route output through SubstitutionOutput::readback. Single-span raw words now pass scanner context into mutable expansion. Typed whole-word splitting and printf shortcut quote changes were tested but reverted after bashdb compatibility regressions.
- Focused verification: cargo check, 9 metadata tests, mixed-fragment, multiple-fragment, literal-IFS, and quoted-empty differential tests passed.
- A temporary child-quote differential probe matched GNU only while the experimental whole-word/printf changes were active; those changes were reverted and the probe was not retained as a passing regression.
- cargo check: passed after 6fe14287.
- cargo test --lib executor::substitution_metadata::tests: 8 passed, including mixed-quote, nested-span, and adjacent raw-fragment cases. GNU/Rubash mixed probe matched for unquoted and double-quoted adjacent fragments.
- cargo test --test cli_tests declare_output: 4 passed.
- scripts/validate-semantic-map.sh: passed after adding substitution_metadata and focused mixed/empty tests to the canonical owner map.
- cargo test --test cli_tests heredoc: 12 passed; no heredoc regression introduced by the fragment owner.
- Final bounded matrix: command_substitution 32 passed / 1 existing bashdb info-files failure; arithmetic 31 passed / 0 failures; printf 6 passed / 0 failures.
- `scan_substitution_spans` now excludes `$((...))` from command-substitution spans; the focused empty-arithmetic differential passes and prevents mixed-fragment expansion from routing arithmetic through command substitution.
- Removing echo/printf shortcuts was tested and reverted: it caused 3 real CLI regressions, proving shortcuts must share the future capture/readback contract rather than simply disappear.

## New Heredoc Differential Evidence

- Round 33 probe: `x=$(printf '\\025'); cat <<EOF; $x; EOF` matches GNU as `15 0a` but Rubash currently emits `5c 0a`. The first destructive owner is `expand_heredoc_body` calling `restore_command_substitution_output` after parameter expansion.
- GNU `subst.c` carries `Q_HERE_DOCUMENT` into the dedicated heredoc scanner; Rubash currently merges parameter payload bytes and legacy heredoc markers into one String before final restoration. A typed heredoc payload owner is required before changing the decoder.

## Backtick Differential Evidence

- Round 46 fix: the backtick variable-value owner now protects raw C0 bytes only when the source word contains a parameter expansion without backtick syntax and the value is not already an owner-tagged payload token. This prevents legacy `\x1a -> backtick` restoration from interpreting payload data.
- GNU/Rubash explicit backtick probes now match for `0x10..0x1c`: each byte survives quoted and unquoted variable use. The focused regression `backtick_command_substitution_preserves_raw_c0_variable_payload` covers `0x14`, `0x15`, `0x1a`, and `0x1f`; assignment and heredoc C0 regressions remain green.
- `0x1d` and `0x1e` remain separate migration risks because Rubash uses them for array/assignment, quoted-heredoc, and compound-assignment protocols. They require typed fragment provenance rather than another global replacement.
- Round 48 isolation: GNU preserves raw `0x1d` through a plain variable, backtick substitution, array element, and unquoted heredoc. Rubash currently consumes the value before those owners materialize it, confirming a collision with the `\x1d` array/assignment protocol rather than a pipe or external process issue.
- Round 49 isolation: GNU preserves raw `0x1e` through a plain variable and array element, while Rubash consumes it during parameter-value materialization; quoted heredoc output also collides with the Rubash `\x1e` quoted-body/compound-assignment protocol. The next fix must introduce a typed payload boundary across `embedded_parameters.rs`, `embedded_mutations.rs`, and array/assignment storage, not alter one decoder.
- Round 54 source mapping: `lexer/word.rs:59-63` encodes quoted parameter words with `\x1d`, while the same byte is consumed by array, assignment, fd, process-substitution, and payload owners throughout `src/executor`. Replacing it with another C0 would only move the collision; the correct migration is an ASCII quote-context marker or typed WORD metadata propagated to all consumers.
- Round 56 inventory: the raw `\x1d` protocol has producers in `lexer/word.rs`, `parser/assignment.rs`, `parser/redirections.rs`, and array/declare storage, with consumers across `command_prepare.rs`, `command_substitution_values.rs`, `parameter_core.rs`, `parameter_words.rs`, `parameter_errors.rs`, `arrays.rs`, fd/read/trap setup, and builtins. This is a cross-subsystem protocol, not a single parameter-expansion bug; migration must replace the protocol at its shared WORD/redirect representation and update all consumers in one change.
- Round 57 inventory: raw `\x1e` has two independent Rubash protocols: lexer quoted-heredoc prefixes (`lexer/mod.rs`, `command_input_scope.rs`, `command_text.rs`, `function_env.rs`) and compound-assignment prefixes (`parser/token_actions.rs`, `assignment_expansion.rs`, `expand_word.rs`, `parameter_core.rs`, declare/setattr/local/temporary assignment owners). GNU keeps these concerns separate: `parse.y:gather_here_documents` queues redirects in `redir_stack` and drains them FIFO (`parse.y:3108-3132`), while `subst.c:param_expand`/assignment handling carries quote state and assignment flags separately. A single C0 replacement cannot model both.

## Remaining Risks

The current implementation now carries context through the single/multiple simple mixed-word and AST/mutable fallback paths, but the general parser fallback still defaults to Unquoted when no parent span metadata is available. Existing C0 sentinel collisions remain in lexer, assignment, heredoc, glob, and backtick paths. A bounded payload probe (`printf '\020'..'\037'` inside command substitution, read back through `od`) matches GNU as `10..1f`, while Rubash currently corrupts the stream (`0x11` is dropped and legacy marker bytes such as `0x15` are restored as shell punctuation). A first owner-tagged ASCII codec experiment was reverted because the actual corruption occurs before the final word exits: the capture buffer is still materialized through legacy marker handling. The durable fix must move typed payload capture earlier, then carry it through lexical quoting and final readback as one contract. The bashdb getopts-long path is fixed: complete quoted arithmetic raw words are now evaluated by the arithmetic owner before test argument expansion, and both the focused conditional regression and bashdb getopts-long test pass. The remaining `info files` mismatch is narrower than the former launcher diagnosis: the target is loaded, but Rubash records `target/bashdb-probe-target.sh` as a relative source path while GNU records `/d/repo/rubash/target/bashdb-probe-target.sh` as its canonical source path. This points to Rubash source-file canonicalization/BASH_SOURCE integration; bashdb fixtures remain unmodified. The bashdb `_Dbg_debugger_name` marker is visible to upstream-script expansion but not to the builtin execution environment, so conditional `pwd` conversion is not a valid ownership boundary. The implemented boundary is narrower: when the bashdb generated launcher is the top-level script, Rubash canonicalizes the debugger-facing `BASH_SOURCE` mirror while retaining the original `__RUBASH_SCRIPT_NAME` for diagnostics. A Pygments `--version` stderr diagnostic is external environment noise.
