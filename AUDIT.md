# Rubash Builtin Gap Analysis - vs GNU Bash (REVISED)

Auditor: researcher (builtin-audit team). REVISED after engineer rewrites (fc.rs, shadowing fix) and live verification of remaining gaps against D:/repo/unixwin-winuxsh/target/debug2/debug/niu.exe. Several earlier HIGH items are actually executor-implemented and work correctly - corrected below.

Method: GNU spec from third_party/bash/builtins/*.def ($SHORT_DOC ... $END); rubash impl from src/builtins/*.rs + executor. Priority: Critical = blocks a live integration; High = correctness hole; Medium = partial/stub; Low = cosmetic/edge.

## File-set note

- 47 *.def files under third_party/bash/builtins/ (task brief said 42; off-by-one).
- Not every *.def has a matching src/builtins/*.rs; several are executor-level.

## Gap table (corrected priorities)

| # | Builtin | Missing Flags/Options | Missing Behavior | Priority |
|---|---------|-----------------------|------------------|----------|
| 1 | fc | Bare fc lacks edit-and-re-execute form (uses -l listing default). String-prefix first not implemented. -e - (execute form) not fully wired. | -s [pat=rep] [command] WORKS (thefuck path). -l/-n/-r/-e + signed first/last + is_number_arg + --help + 14 tests all present. Edit-and-re-execute form (FCEDIT/EDITOR/vi temp file) not implemented but unused by thefuck. | DONE (was Critical) |
| 2 | history | -s (append) still STUB. -a/-n/-r/-w HISTFILE I/O still STUB. -p (expansion) STUB. * on modified, HISTTIMEFORMAT missing. | -c clear WORKS (executor). -d delete WORKS (executor). bare history + history N work (numbers only). -s does NOT append in isolation. No HISTFILE read/write/append. No history expansion. | Medium (was High) |
| 3 | alias | None missing. | AL_EXPANDNEXT tracked. -p and unalias -a work. | Low |
| 4 | declare | -f -F (function) absent. -g -I parsed no-op. | No function-scope interaction. +r accepted in some contexts where bash forbids. | Medium |
| 5 | echo | -n/-e/-E work. Full escape set implemented. | xpg_echo default inverted vs interactive bash. | Medium |
| 6 | exec | -a/-c/-l parsed. | EXEC-REPLACES CORRECTLY (live-verified: exec echo REPLACED; echo AFTER -> only REPLACED). argv[0]/login-dash partially wired. | Low (was High) |
| 7 | kill | %jobspec (%1, %fg, %bg) and negative pids (-N) are the ONLY gaps. | %1 returns RC=0 but does NOT kill (silent no-op). %fg/%bg rejected (RC=1). -TERM -1 rejected (RC=2). Everything else works: -s/-n/-l/-L, SIG*, 0/EXIT, 0-probe, STOP/TSTP/CONT, async mailbox. | Medium (was Low) |
| 8 | pushd/popd/dirs | None missing. | Matches bash. | Low |
| 9 | cd | -@ (xattr-as-dir) missing. | cdable_vars, CDPATH, HOME, cd - all work. | Low |
| 10 | complete | -abcdefgjksuvprDEI + -o/-A/-G/-W/-F/-C/-X/-P/-S all parsed. | complete -p prints nothing; specs neither stored nor consulted. complete -r removes nothing. compgen works. compopt always errors. -F/-C not wired. | High (confirmed) |
| 11 | test / [ | All operators present. | -r/-w/-x -O/-G on non-unix return exists. | Low |
| 12 | printf | -v var, all %specs, dynamic width, reuse loop. | %q quoting differs slightly. | Low |
| 13 | set/unset | All flags. | Positional-param assignment on --/- is no-op. unset fully implemented. | Medium |
| 14 | shopt | -pqsu and -o work. | globstar, extglob, nullglob, dotglob, nocasematch ALL WORK (executor consumes them). Remaining bookkept-but-unconsumed options are rare (extdebug, direxpand, histreedit, histverify, cdspell, noclobber shopt, progcomp_alias, hostcomplete). | Low (was High) |
| 15 | trap | -l/-p/-P, EXIT/DEBUG/ERR/RETURN, SIG*, clear, subshell reset. | ERR/EXIT/DEBUG traps ALL FIRE correctly (live-verified). Table + execution work. Usage string lacks -P (cosmetic). | Low (was High) |
| 16 | wait | -f/-n/-p parsed. | wait $PID WORKS (live-verified WAITED_RC=0). -p var assignment not wired. Job/pid resolution via executor. | Low (was High) |
| 17 | bind | All flags accepted. | line-editing-not-enabled warning + success; no readline binding. Parse-only stub. | Medium |
| 18 | caller | Level, current-frame, frame-walk. | Source display differs slightly at level>=1. | Low |
| 19 | eval | No options. | Spec-correct. | Low |
| 20 | exit | [n], numeric-required, normalization. | Wraps to [0..255]. | Low |
| 21 | hash | -l/-r/-d/-t/-p present. | -p inline parsing bug (mis-shifts). Hash table not consulted by lookup. | Medium |
| 22 | help | -d/-m/-s. | -d and -m modes may be unimplemented. | Low |
| 23 | jobs | -l/-p wired; -x dispatches. | -n/-r/-s parsed but ignored. | Medium |
| 24 | let | Executor-level arithmetic. | set -e interaction unverified. | Low |
| 25 | mapfile/readarray | Full flags at executor level. | -C/-c/-u/-clearing unverified. | Low-Medium |
| 26 | read | -a/-d/-e/-i/-n/-N/-p/-r/-s/-t/-u handled. | -e/-i no-op without readline; fractional -t unverified. | Medium |
| 27 | disown | -a/-r/-h parsed. | Upstream. | Low |
| 28 | break/continue/return/builtin | Keyword-level executor. | Nesting beyond 1 verified via parser. | Low |
| 29 | getopts | Full optstring handling. | OPTARG/OPTIND in executor. | Low |
| 30 | shift | [n] with numeric/out-of-range. | Does NOT validate n > $#. | Medium |
| 31 | source / . | Full: PATH/sourcepath, -p, positional, inline, pipe, alias-in-source. | Trap/unwind TODO. | Low |
| 32 | suspend | -f parsed. | Gives proper cannot suspend: no job control error and exits. Works correctly for non-interactive. | Low (was Medium) |
| 33 | times | No options. | Prints hardcoded 0m0.000s; never reads process clock. | Medium |
| 34 | type | -a/-f/-p/-t/-P + long forms. | -f (suppress function) parsed but ignored. Aliases not reported. coproc/keyword not by type -t. | Medium |
| 35 | ulimit | Full flag matrix. | Values stored as bookkeeping - never setrlimit. Cosmetic. | Medium |
| 36 | umask | -p/-S + octal/symbolic. | Never calls umask(2) - bookkeeping only. | Medium |
| 37 | enable | -a/-n/-p/-s/-f/-d + DISABLED_BUILTINS. | Dynamic loading no-op. enable -n correctly disables (falls through to external). | Low |
| 38 | command | -p/-v/-V present. | Uses rubash PATH resolver. | Low |
| 39 | export/readonly/local | Full attribute tracking. | export -f / readonly -f (functions) no-op. | Medium |
| 40 | sudo | Windows passthrough. Not bash builtin. | - | n/a |
| 41 | true/false/: | Constant 0/1/0. | Ignore trailing args. | None |
| 42 | compgen/compopt | See row 10. compgen covers 20+ actions. compopt always errors. | - | see row 10 |
| 43 | logout/coproc/select/[[/]]/{} /time | Parser/executor keywords. | No matching *.def. Out of scope. | n/a |

## Summary - corrected priority order

The ONLY remaining HIGH-severity gap affecting common scripts: complete/compopt (tab completion broken end-to-end).

Remaining real gaps by priority:
1. complete/compopt - HIGH. Parse-only; no completions for any command.
2. history -s append + HISTFILE I/O (-a/-n/-r/-w) - MEDIUM. Cross-session persistence broken; -c and -d work.
3. kill job-spec (%1, %fg, %bg) + negative pid (-N) - MEDIUM. %1 returns 0 but silently does not kill.
4. declare -f/-F, echo xpg_echo default, set positional-param, shift n>$#, hash -p parsing, jobs -n/-r/-s, bind (no readline), read (-e/-i), export/readonly -f, type -f, ulimit/umask (bookkeeping), times (hardcoded) - MEDIUM/LOW.
5. Suspended/HIGH items DEMOTED after live verification: shopt (globstar/extglob/nullglob/dotglob/nocasematch work), trap (ERR/EXIT/DEBUG fire), exec (replaces), wait (waits), suspend (proper error). These work via executor-level implementation despite .rs stubs.

Shadowing fix (command_substitution_values.rs:485-490) confirmed comprehensive - all 67 is_shell_builtin_name entries protected; 11 colliding builtins all invoke shell builtin in $(); PATH-order-proof; enable -n falls through correctly. See SHADOW_AUDIT.md.

## Sources of truth used

- D:/repo/rubash/third_party/bash/builtins/*.def - 47 files, $SHORT_DOC ... $END blocks.
- D:/repo/rubash/src/builtins/*.rs - all top-level builtin modules read.
- D:/repo/rubash/src/executor/{mapfile_builtin,builtin_direct_command,command_dispatch_{primary,late,no_alias,no_alias_late},builtin_names,job_builtins,support_names,trap_stack_builtins,command_substitution_values,pipeline_exec,pipeline_stages,external_inner,lookup_paths}.rs - executor-level implementations.
- Live verification against D:/repo/unixwin-winuxsh/target/debug2/debug/niu.exe.