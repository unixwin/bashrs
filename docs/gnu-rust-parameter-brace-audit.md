# GNU/Rubash Semantic Audit: Parameter Braces

> Scope: GNU Bash 5.3 parameter-brace scanning and quote preservation.
> Evidence: third_party/bash/subst.c, parse.y, current Rubash sources and focused tests.

## Finding

GNU Bash has one brace-scanning semantic model spread across parser and expansion code. Current Rubash has multiple independent scanners without the GNU dolbrace state. A fix in one stage can change the boundary or quote metadata seen by another stage.

The highest-risk reproduction is remote issue #62 / posixexp2:

    (echo 2 \"${IFS+'}'z}\")

The required Bash output is 2 '}'z. The unquoted companion must continue to pass:

    (echo 1 ${IFS+'}'z})

## GNU Contract

GNU subst.c::extract_dollar_brace_string initializes DOLBRACE_PARAM, saves state for nested parameter expansions, enters DOLBRACE_PARAM for nested expansions from WORD/QUOTE states, restores state at the matching brace, and advances past escaped characters as one unit.

GNU parse.y::parse_matched_pair transitions parameter state through PARAM, OP, WORD, QUOTE, and QUOTE2. In POSIX outer-double-quote mode, single-quote treatment depends on the current state. Quote removal is later and must not destroy raw quote information needed by parameter-word expansion.

The implementation must keep these concerns separate:

1. Structural boundary scanning.
2. Expansion-word quote metadata.
3. Final quote removal.

A single single/double toggle or early sentinel substitution is not an equivalent model.

## Current Duplicate Owners

| GNU contract | Current Rubash entry points | Risk |
|---|---|---|
| Parameter boundary in lexer | src/lexer/skip.rs::skip_braced; src/lexer/quotes.rs::copy_braced_parameter_after_dollar | Different quote and nesting rules |
| Character-slice boundary | src/lexer/brace_scan.rs::skip_braced_parameter_in_chars | Separate scanner, no dolbrace state |
| Parser-side boundary | src/parser/parameter_expansion.rs | Separate nested command/brace handling |
| Pattern-side boundary | src/parser/brace_expansion.rs; extglob_pattern.rs; pathname_pattern.rs | Repeated local scanners |
| Parameter-name collection | src/executor/command_subst_helpers.rs | No GNU operator-word state |
| Whole-word matching | src/executor/parameter_ops.rs::matching_parameter_brace | Ad-hoc replacement-context heuristic |
| Operator-word preparation | src/executor/command_prepare.rs; parameter_words.rs | Quote metadata can be consumed too early |
| Embedded expansion mutation | src/executor/embedded_parameters.rs | Receives already-decoded fragments |

## Required Shared API

Introduce one low-level scanner independent of execution:

- Input: source slice beginning at ${, plus outer quote/POSIX context.
- State: Param, Op, Word, Quote, Quote2.
- Stack: saved state for nested ${...}.
- Output: matching end offset, raw span, and quote metadata events.
- Escapes: consume backslash/next character as one structural unit.
- Quote removal: never performed by the scanner.

The first migration should replace lexer and executor boundary scanners with adapters over this API. Parser pattern scanners can follow after focused behavior is stable.

## Required Evidence

- ${IFS+'bar} closes at its outer brace and does not consume the next token.
- ${IFS+'}'z} preserves the quoted brace in the operator word.
- The POSIX outer-double reproduction prints 2 '}'z.
- The unquoted reproduction keeps existing behavior.
- ${v/$'\\''/x} preserves ANSI-C quote structure.
- Nested ${A[${i}]} keeps the outer closing brace.
- Braces in bracket patterns and escaped braces remain correct.
- Existing lexer multiline single-quote and pipeline regressions remain green.

## Audit Rules

- Every new brace scanner must map to this contract or be explicitly classified as a different grammar.
- Do not alter upstream expected files to close a diff.
- Keep suite artifacts under target/issue-suites/results/.
- Preserve unrelated dirty worktree changes.
- Run scripts/validate-semantic-map.sh after updating the ownership map.

## Migration Order

The first adapter should be `src/executor/parameter_ops.rs::matching_parameter_brace`, because it has focused unit tests and is already the shared consumer for whole-word checks. The next adapter is `src/lexer/brace_scan.rs::skip_braced_parameter_in_chars`. After those are stable, migrate `src/lexer/quotes.rs::copy_braced_parameter_after_dollar` and `src/executor/command_subst_helpers.rs::collect_braced_parameter_name`. Only then migrate the main lexer skip path, parameter-word consumers, and quote-sentinel restoration.

The shared API is named conceptually as `ParameterSpan`, `DolbraceState`, `BraceContext`, and an opaque structural skipper. The current prototype uses `BracedScan` as the temporary span name while adapters are migrated. It must not evaluate values, remove quotes, or generate sentinels.

## Migration Status

`matching_parameter_brace`, `skip_braced_parameter_in_chars`, the main lexer brace skip path, and `copy_braced_parameter_after_dollar` now use the shared structural scanner through adapters. Matcher replacement context is now an explicit scanner option instead of an input slash bypass; the legacy malformed-input fallback remains scheduled for removal after an explicit `Unclosed` result is added. Focused lexer, executor, scanner, and unquoted POSIX regressions pass. The quoted `posixexp2` output is now `2 '}'z`. The root cause was a second `remove_shell_quotes` pass in `expand_command_word` that reinterpreted the already quote-aware `\x1d` parameter-word result; the cleanup now skips that pass for quoted parameter words. The scanner remains structural-only.
