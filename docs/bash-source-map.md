# Bash Source Map

## Semantic Map v2

The canonical migration map is `docs/semantic-ownership.tsv`. It records
semantic ownership rather than pretending that one GNU C file has one Rust
translation. Its columns are:

| Column | Meaning |
|---|---|
| GNU source family | GNU C, `.def`, `.y`, or upstream test family |
| Semantic contract | Observable Bash behavior owned by the family |
| Rust owner | One or more Rust semantic owners, separated by `;` |
| Compile status | `active`, `unreferenced`, or `missing` |
| Implementation status | `real`, `partial`, `scaffold`, `bridge`, `deferred`, or `host-owned` |
| Suite evidence | Rust and upstream test families that exercise the owner |
| Next gate | Evidence required before the owner can be promoted |

The map is checked by `scripts/validate-semantic-map.sh`. A target file's
existence is not evidence of migration: `real` requires an active owner,
non-placeholder implementation, and named test evidence. One GNU family may
map to several owners, and several families may map to one semantic kernel.

The older file-by-file inventory below remains provenance data. New migration
work must update the v2 map first and may use the appendix for source lookup.

## Placeholder Audit

Run `scripts/audit-rust-placeholders.sh` to inspect Rust files that contain
only GNU provenance comments. The report distinguishes files declared by the
current module tree from unreferenced files, and separates host/deferred areas
(`input`, `readline`, `locale`, `sys`, completion, and history) from old
semantic-owner candidates.

`Code=0` is not an implementation status. An unreferenced file is removable
only after its provenance is represented in this map or the implementation
inventory. A duplicate-owner candidate is not automatically complete: the
replacement owner still needs behavior and suite evidence before it can be
marked `real`.

This map keeps Rubash implementation work traceable to GNU Bash 5.3 sources
without forcing a file-for-file port. The `Status` column describes whether the
Rubash module should exist now or later.

## Upstream Inventory

The pinned GNU Bash submodule currently contains 1603 tracked files. The files
that most directly shape Rubash implementation are the C sources, headers,
builtin definitions, and parser grammar:

| Group | Count | Notes |
|---|---:|---|
| Total tracked files | 1603 | Full GNU Bash source tree, including docs, tests, translations, build support, and examples. |
| `.c` files | 301 | C implementation files across the root, `builtins/`, `lib/`, examples, and support tools. |
| `.h` files | 141 | C headers and generated/config headers. |
| `builtins/*.def` files | 43 | Bash builtin command definitions. |
| `.y` files | 2 | Parser grammars, including `parse.y`. |
| C/header/def/parser total | 487 | The main implementation-shaped inventory Rubash should track semantically. |
| `tests/` files | 738 | Upstream conformance and regression suite data. |
| `lib/` files | 316 | Readline, glob, tilde, sh portability helpers, malloc, intl, termcap. |
| `builtins/` files | 56 | Builtin definitions plus helper code. |
| `doc/` files | 37 | Manual/reference documentation. |

This document maps those files at subsystem granularity. The file-by-file owner
map lives in `docs/bash-implementation-inventory.md`; it assigns every
implementation-shaped GNU Bash file to a Rubash target module or an explicit
skip category. When a Rubash module is added or moved, update both maps as
needed.

| GNU Bash source | Rubash module | Status | Notes |
|---|---|---:|---|
| `parse.y`, `parser.h`, `y.tab.c`, `y.tab.h` | `src/parser/` | Now | Parser grammar reference only; do not mirror generated `y.tab.*`. |
| `command.h`, `make_cmd.c`, `copy_cmd.c`, `dispose_cmd.c`, `print_cmd.c` | `src/parser/ast.rs` | Now | Rust AST should model command semantics, not C allocation helpers. |
| `subst.c`, `subst.h` | `src/expand/parameter.rs`, `src/expand/command.rs` | Now | Parameter, command, arithmetic, quote removal, and word expansion logic. |
| `braces.c`, `bracecomp.c` | `src/expand/braces.rs` | Now | Brace expansion can be implemented independently and tested early. |
| `pathexp.c`, `lib/glob/glob.c`, `lib/glob/strmatch.c` | `src/expand/pathname.rs` | Now | Pathname expansion and shell pattern matching. |
| `lib/tilde/tilde.c` | `src/expand/tilde.rs` | Now | Needed by `cd`, assignments, and word expansion. |
| `execute_cmd.c`, `execute_cmd.h`, `eval.c` | `src/executor/command.rs` | Now | Main command execution flow. Keep high-level orchestration here. |
| `redir.c`, `redir.h` | `src/executor/redirection.rs` | Now | File descriptor and redirect semantics. |
| `findcmd.c`, `hashcmd.c`, `hashlib.c` | `src/executor/path.rs` or `src/shell/hash.rs` | Later | Command lookup and hashing after basic execution works. |
| `variables.c`, `variables.h` | `src/shell/variables.rs` | Now | Shell variables, exported environment, special parameters. |
| `flags.c`, `shell.c`, `shell.h` | `src/shell/options.rs`, `src/shell/status.rs` | Now | Shell options, invocation mode, exit status, runtime state. |
| `builtins/*.def`, `builtins/common.c` | `src/builtins/` | Now | Implement per builtin where useful, but group small builtins pragmatically. |
| `test.c`, `builtins/test.def` | `src/builtins/test.rs` | Now | `test` and `[` behavior should share one implementation. |
| `alias.c`, `alias.h`, `builtins/alias.def` | `src/shell/alias.rs` | Later | Needs parser/input integration before it is useful. |
| `array.c`, `array2.c`, `arrayfunc.c`, `assoc.c` | `src/shell/arrays.rs` | Later | Add after scalar variables and parameter expansion are stable. |
| `jobs.c`, `nojobs.c`, `jobs.h` | `src/jobs/` | Later | Requires process groups, terminal control, and signal semantics. |
| `trap.c`, `sig.c`, `siglist.c` | `src/jobs/signals.rs` or `src/shell/signals.rs` | Later | Implement with job control or script traps, not before. |
| `input.c`, `bashline.c`, `lib/readline/*` | `src/input/` or external line editor | Later | Prefer crate-backed line editing before considering Bash readline parity. |
| `pcomplete.c`, `pcomplib.c`, `builtins/complete.def` | `src/complete/` | Later | Depends on readline/input and shell metadata. |
| `bashhist.c`, `lib/readline/history.c` | `src/history.rs` | Later | Interactive-only feature. |
| `locale.c`, `bashintl.h`, `po/`, `lib/intl/` | `src/locale.rs` | Defer | Not needed for early conformance. |
| `lib/sh/*` | `src/sys/` or standard library replacements | Selective | Most files are portability helpers; use Rust std/nix equivalents instead of porting. |
| `tests/*.tests`, `tests/*.right`, `tests/*.sub` | `scripts/run-bash-upstream-tests.sh` | Now | Keep upstream tests in the submodule and run the project harness for compatibility baselines. |

## Compatibility Target

The target is GNU Bash 5.3 observable behavior, including default Bash mode and
POSIX mode. Bash itself documents differences between default mode and POSIX
mode in `third_party/bash/POSIX`, and user-visible version differences in
`third_party/bash/COMPAT`.

Rubash progress should be measured by:

- Rust unit and integration tests for local implementation details.
- GNU Bash upstream `tests/run-*` progress.
- Focused differential tests against GNU Bash for newly implemented behavior.
