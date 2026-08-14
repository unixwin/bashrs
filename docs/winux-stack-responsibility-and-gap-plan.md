# Winux Stack Responsibility and Gap Plan

> Scope: RUBASH semantic ownership and its Windows host boundary.
> Status: architecture and test-execution handoff; no Rust or C/C++ implementation changes.
> Evidence basis: current repository HEAD 88f72fc, src/jobs/table.rs, src/executor/fd_table.rs, src/executor/external_setup.rs, src/executor/redirection.rs, src/shell/variables.rs, src/executor/upstream_scripts.rs, and the GNU Bash sources under third_party/bash. This is an implementation audit, not a claim that every listed path is complete.

## 1. Three-layer contract

The stack has three owners. A behavior belongs to the layer that owns its observable state, not to the layer that happens to launch a process or display output.

| Layer | Owns | Must not own |
|---|---|---|
| **WINUXSH** | reedline/REPL, history, completion and autosuggest, prompt/theme/git status, config and .winuxshrc/.winshrc, plugin lifecycle, Windows UX, CLI mode, host lifecycle, and command-not-found presentation | Bash parsing, expansion, shell variables, arrays, shell fd state, pipeline/job semantics, traps, coproc, process substitution, or Bash diagnostics/status rules |
| **RUBASH** | Bash lexer/parser/AST; quoting and all word expansions; Bash builtins; variables, arrays, associative arrays, nameref and positional parameters; redirection, heredoc, here-string and shell fd state; pipelines, subshells and command substitution; $?/$!/PIPESTATUS, pipefail, errexit and nounset; jobs, wait, jobspecs and disown; trap, coproc and process substitution; Bash diagnostics and exit statuses | reedline behavior, interactive history storage, prompt rendering, host plugin policy, Windows process/handle implementation details |
| **WinuxCmd** | external command implementations; filesystem/path/process/handle/pipe primitives; Windows child creation, waiting, polling and termination; process enumeration and Windows-specific limitations | same-name RUBASH builtins, shell-state mutation, parsing/expansion, fd/job bookkeeping, trap delivery policy, Bash diagnostics, or a second shell executor |

The dependency direction is: RUBASH semantic execution -> WinuxCmd backend primitives, with WINUXSH embedding and hosting RUBASH. WINUXSH may supply an interactive input/output/session contract; it does not reinterpret a parsed Bash command.

## 2. Why WINUXSH current role is correct

The shell product layer is the right home for REPL policy and user experience. History persistence, reedline editing, completion, autosuggest, prompt/theme rendering, git status, config discovery, plugins, and command-not-found presentation are host concerns. They depend on an interactive session and should be replaceable without changing noninteractive Bash semantics.

Fast paths in WINUXSH are acceptable when they are host dispatch shortcuts: recognizing a known external command, selecting a completion provider, avoiding an interactive redraw, or choosing a host lifecycle path. A fast path must return to the same RUBASH semantic contract for any command that can observe or mutate shell state. It must not become a second parser/executor that handles quoting, expansions, redirections, pipelines, backgrounding, builtins, jobs, traps, or status aggregation differently. A shortcut that bypasses RUBASH is therefore limited to an explicitly documented host-only operation or an external command whose arguments are already finalized by RUBASH.

## 3. Same-name commands and builtin precedence

WinuxCmd command names such as cd, export, set, read, jobs, wait, kill, trap, exec, source, printf, and every other Bash builtin cannot replace the RUBASH builtin merely because an executable has the same spelling.

The rule is:

1. RUBASH resolves and executes a Bash builtin in the current shell context.
2. The builtin may mutate VariableStore, cwd, shell options, fd state, traps, jobs, positional parameters, or status state.
3. Only a command that is semantically external after Bash resolution is sent to WinuxCmd.
4. An explicit external invocation such as command or a path-qualified dispatch must still preserve Bash resolution, redirection, status, and diagnostic rules before backend launch.

A WinuxCmd implementation can provide the primitive needed by a builtin, for example process termination for RUBASH kill, directory change primitives for RUBASH cd, or handle duplication for redirection. It does not own the builtin option parsing, shell-state mutation, jobspec resolution, or output/status behavior.

## 4. RUBASH subsystem status

| Subsystem | Current owner/evidence | Status | Remaining gate |
|---|---|---|---|
| Lexer/parser/AST | src/lexer, src/parser; GNU parse.y, command.h, error.c | active, partial | strict parse errors, malformed constructs, heredoc collection and status parity |
| Word expansion | src/expand, executor expansion modules; GNU subst.c, braces.c, glob family | active, partial | quoting/IFS, parameter edge cases, command/arithmetic/brace/tilde/pathname ordering |
| Builtins | src/builtins; GNU builtins/*.def | active, uneven | option parsing, diagnostics, status and shell-state parity across the builtin family |
| Variables/arrays/nameref | src/shell/state.rs, src/shell/variables.rs, src/shell/arrays; GNU variables.c, array*.c, assoc.c | typed owner with legacy adapter, partial | route remaining executor reads/writes through typed VariableStore; remove delimiter encoding; complete attributes and nameref semantics |
| Fd/redirection | src/executor/fd_table.rs, redirection.rs, external_setup.rs; GNU redir.c, redir.h | active, partial; FdTable is semantic source for migrated paths | finish ordered dup/move/close, dynamic descriptors, device behavior and lifetime; remove remaining environment fallback in external setup |
| Heredoc/here-string | lexer heredoc modules plus redirection/read paths; GNU subst.c, redir.c | partial | ordered collection, quoted delimiters, command substitution nesting, streaming large input, no hangs |
| Pipeline/subshell/command substitution | executor command/eval modules; GNU execute_cmd.c, eval.c, subst.c | partial | aggregate status, fd ownership, subshell isolation, pipefail and signal/exit propagation |
| Status/options | shell state/options and executor; GNU flags.c, error.c | partial | $?, $!, PIPESTATUS, errexit/nounset and arithmetic/status propagation |
| Jobs/wait | src/jobs/table.rs, src/executor/job_builtins.rs; GNU jobs.c, nojobs.c, builtins/wait.def | partial foundation | pipeline registration, completion retention and basic current/previous jobspec parsing exist; add per-process/aggregate transitions, stopped/continued/killed handling, fg/bg, wait -n, notifications and cleanup |
| Trap/signals | trap builtin/executor; GNU sig.c, trap.def | partial | delivery timing, status interaction, Windows interrupt/termination capability mapping |
| Coproc/process substitution | parser coproc, FdTable, JobTable, external setup; GNU coproc_command.c, process_substitution.c | real endpoint path plus remaining upstream bridge | finish endpoint lifetime, reaping, shell-exit cleanup and structured status; remove interception only after direct semantic coverage is verified |
| Interactive editing/history | host-owned / src/input/readline boundary; GNU lib/readline/*, bashhist.c | deferred, not a current RUBASH gap | define host/editor and noninteractive boundary |

## 5. Background control judgement

RUBASH already has a meaningful asynchronous foundation: executor background children and status, a JobTable, jobs/wait/disown paths, partial pipeline concurrency, and some Ctrl+C handling. It is inaccurate to call background execution completely absent.

It is equally inaccurate to call job control complete. The current JobTable already registers a pipeline as one JobEntry, maps pids to jobs, retains completed statuses, resolves the basic %+/%- forms, and records coproc endpoints. Compared with GNU jobs.c, nojobs.c, and execute_cmd.c, the missing closure includes authoritative per-process and aggregate transitions, stopped/continued/killed states, full jobspec matching and current/previous maintenance, jobs -n notifications, fg/bg behavior, terminal/process-group Windows behavior, safe wait retention/bookkeeping, coproc/process-substitution cleanup, shell-exit escalation, and consistent $!/PIPESTATUS/wait -n/signal statuses. The correct status is **basic async and a partial job table exist; complete job control is not closed**.

## 6. GNU source families and RUBASH gaps after excluding host layers

| GNU family | RUBASH owner | Gap to close |
|---|---|---|
| parse.y, command.h, copy_cmd.c, dispose_cmd.c, error.c | lexer/parser/AST/error modules | Bash grammar strictness, malformed syntax rc=2, AST lifetime and exact diagnostics |
| subst.c, expr.c, braces.c, glob library | expand and executor expansion modules | expansion ordering, quote removal, IFS/word splitting, arithmetic errors, brace/tilde/pathname edge cases |
| variables.c, array.c, array2.c, arrayfunc.c, assoc.c | VariableStore and arrays | typed values, sparse/assoc arrays, attributes, nameref, export view, positional/status state |
| redir.c, redir.h | FdTable/redirection/read paths | ordered application, dynamic fd allocation, dup/move/close, undo/lifetime, device and diagnostics |
| execute_cmd.c, eval.c | executor command/eval | pipeline/subshell aggregate, external setup, fd inheritance, command substitution, status and pipefail |
| jobs.c, nojobs.c, builtins/wait.def, builtins/fg_bg.def, builtins/jobs.def | JobTable and job builtins | complete identity/state/jobspec/notification/wait/terminal contract |
| sig.c, builtins/trap.def, builtins/kill.def | trap, kill, signal adapters | Bash delivery/status semantics over Windows capabilities; builtin/backend separation |
| coproc_command.c, process_substitution.c | coproc parser, FdTable, JobTable | async endpoints, endpoint fd variables, cleanup, reaping and status |
| builtins/*.def, builtins/common.c, bashgetopt.c | src/builtins | builtin option/error/output/status parity and shell-state mutation |
| findcmd.c, hashcmd.c, hashlib.c | executor path/hash | command resolution and hash state without delegating Bash semantics to WinuxCmd |
| flags.c, general.c, shell.c | shell options/general state | option transitions, exit policy, invocation state, noninteractive contract |

These are semantic gaps only. GNU Readline/history/completion, prompt/UI, locale/catalog portability, and Windows platform/process primitives are excluded from the RUBASH gap list when they are host responsibilities.

## 7. Migration order

The migration should preserve one source of truth at each step:

1. **FdTable**: continue the existing migration. FdTable is already the semantic source for migrated redirection/output paths and external setup can materialize its state. Finish virtual fd identity, ordered redirection, dup/move/close, inheritance, device mapping, and structured fd errors; then remove the remaining environment-key fallback.
2. **Script parser/heredoc**: make script parsing and heredoc collection produce ordered, typed command input; cover quoted delimiters, multiple heredocs, nested substitution, here-string and large streaming input.
3. **VariableStore**: route assignment, expansion reads, arrays/assoc/nameref, export, positional parameters, $?, $!, and PIPESTATUS through typed state. Remove delimiter-encoded internal mutation paths.
4. **Structured result**: introduce the missing command/process/pipeline result contract with exit code, signal/stop/continue reason, per-process statuses, aggregate status, captured endpoint ownership, and diagnostics data. Existing status fields and snapshots are not yet a complete replacement for this result model. Do not pass preformatted upstream output as a semantic result.
5. **JobTable**: register every background process, pipeline aggregate, coproc and process-substitution helper with stable identity and lifecycle transitions; implement jobspec resolution, wait retention, notifications, fg/bg, cleanup, and shell-exit escalation.

This order prevents job bookkeeping from depending on an fd mirror, textual output bridge, or untyped environment state.

## 8. Upstream output bridge removal gate

The src/executor/upstream_scripts* bridge is compatibility scaffolding. Its output must be removed only when the corresponding behavior is implemented by RUBASH semantics and covered by focused tests plus the relevant GNU suite slice. A passing bridge-backed runner is evidence that a harness path works, not evidence that the semantic owner is complete.

For each bridge handler, record: the GNU source family; the real Rust owner; a direct behavioral reproduction; structured-result/fd/job coverage; a focused Rust regression; a native GNU comparison; and a bounded upstream suite result. Remove one handler at a time, rerun the same slice, and verify no output/status/diagnostic behavior was supplied by the bridge. Coproc and process-substitution bridges additionally require endpoint lifetime, reaping, and shell-exit cleanup tests.

## 9. Explicitly out of the current RUBASH gap

The following GNU families are not missing RUBASH semantics merely because they exist in the Bash tree:

- GNU Readline implementation and editing commands: lib/readline/*, interactive bashline.c behavior, keymaps, terminal redraw, kill/yank editing, and completion UI. RUBASH only needs a stable noninteractive/parser boundary; WINUXSH owns the editor.
- Interactive history persistence and policy: bashhist.c/history integration, history storage, and editor lifecycle are WINUXSH concerns in this stack. Do not turn GNU history plumbing into a current RUBASH gap; only a separately agreed Bash command contract would change that boundary.
- Completion generation and autosuggest UI, prompt/theme/git status, plugins, config files, and command-not-found presentation. These are WINUXSH product concerns.
- Locale catalogs and portability wrappers such as lib/intl/*, locale message selection, and Windows encoding/catalog plumbing, except where RUBASH must expose a stable diagnostic contract to the host.
- Windows filesystem, process, handle, pipe, enumeration, and termination implementation files. These are WinuxCmd primitives behind a RUBASH contract.
- GNU configure/build portability headers and platform shims that do not define an observable Bash semantic owned by RUBASH.

The exclusion does not permit either host layer to reimplement Bash parsing or shell state. It only prevents UI, locale, and platform implementation work from being misclassified as RUBASH compatibility debt.

## 10. Test evidence policy for this handoff

Historical suite numbers remain historical. A current turn must cite only artifacts written during that turn under target/issue-suites/results/ or target/bash-upstream-tests/, with command, timeout, exit code, pass/fail/diff/timeout classification, high-signal files, harness classification, and before/after process checks. A runner that exits zero while reporting diffs is not a semantic pass, and a Harness process-launch failure is not a RUBASH test failure.
