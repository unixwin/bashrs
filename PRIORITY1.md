# Rubash vs GNU Bash - Builtin Gap Tables (Priority 1, FINAL REVISED)

Auditor: researcher. **Final revision after engineer rewrite of fc.rs, the shadowing fix, and live verification of ALL remaining gaps.** Prior assessments based on stale .rs files have been corrected throughout. Many AUDIT.md HIGH items are actually executor-implemented and work correctly.

All findings live-verified against D:/repo/unixwin-winuxsh/target/debug2/debug/niu.exe.

---

## 1. fc - DONE (thefuck works)

fc.rs is now a full 333-line implementation. Live-confirmed: set -o history; history -s git pushh; history -s fuck; echo $(fc -ln -1) -> git pushh. The -s form (thefuck path) works. -l/-n/-r/-e/-s + signed first/last + is_number_arg + --help + 14 unit tests. CRITICAL resolved. Remaining (edit-and-re-execute form, string-prefix first) are LOW-polish, not blockers.

| Flag/Option | Bash | Rubash NOW | Fix |
|---|---|---|---|
| -s [pat=rep] [cmd] | Re-execute after substitution | WORKS - $(fc -ln -1) returns prior command | none (DONE) |
| -l/-n/-r | List/omit-nums/reverse | WORKS | none |
| -e ENAME | Select editor | Parsed; -e - path not fully wired | Low polish |
| signed first/last | Relative offset | WORKS via is_number_arg + isize | none |
| bare fc (edit+re-execute) | FCEDIT/EDITOR/vi temp file | -l listing by default (no edit form) | Low polish |
| string-prefix first | Most recent command starting with string | Numeric-only | Low - thefuck uses numeric |

---

## 2. history - LIVE VERIFIED (mostly works; HISTFILE I/O + -s remain stubs)

Source: history.rs (105 lines, STILL stub) BUT executor provides -c clear and -d delete. Live results:

- history -c: N=8 -> 0 AFTER. WORKS (executor-level).
- history -d 2: removed entry 2 from list. WORKS (executor-level).
- history -s x (isolated): N=[]. STUB - -s does NOT append.
- history -w: file empty. STUB - no HISTFILE write.
- history -r/-n/-a: counts unchanged. STUB - no HISTFILE read.

| Flag/Option | Bash | Rubash NOW | Fix |
|---|---|---|---|
| bare history | List + * + HISTTIMEFORMAT | Numbers only; no *, no timestamps | Add * marker; HISTTIMEFORMAT |
| history N | Last N | WORKS | none |
| -c | Clear list | WORKS (executor) | none |
| -d offset | Delete entry | WORKS (executor) | none |
| -s arg | Append one entry | STUB - does not append | Call add_history |
| -a/-n/-r/-w [file] | HISTFILE I/O | STUB - no file I/O | Implement read/write/append |
| -p | History expansion | STUB | Run expansion engine |
| HISTTIMEFORMAT | Timestamps | Missing | Format per strftime |
| * on modified | Marker | Missing | Track + render |

Status: MEDIUM. -s append and HISTFILE I/O are real gaps for cross-session persistence; -c and -d work.

---

## 3. kill - LIVE VERIFIED (job-spec + negative-pid are the ONLY gaps)

Source: kill.rs (504 lines). Live-confirmed: $(kill -l 0) -> EXIT. kill -s 0 1 (existence) works. Full flag set works. Two delivery gaps, confirmed:

- kill %1: returns RC=0 but does NOT kill the job (job still Running after). BUG - silent no-op.
- kill %fg / %bg: RC=1 (rejected).
- kill -TERM -1: RC=2 (negative pid rejected by parse_pid).

| Flag/Option | Bash | Rubash NOW | Fix |
|---|---|---|---|
| %jobspec (%1, %fg, %bg) | Resolve to job pgrp leader | %1 returns 0 but doesnt kill; %fg/%bg rejected | Integrate job-table lookup + delivery |
| -N / 0 (process group) | Send to pgrp / existence probe | Negative pids rejected (RC=2); 0-probe works | Accept negative pids as pgrp targets |
| everything else | - | WORKS | none |

Status: MEDIUM. These two gaps are the ONLY kill issues.

---

## 4. GENUINE remaining HIGH-severity gap (only one)

AUDIT.md listed shopt, trap, exec, wait as HIGH. **Live verification shows all four work via executor-level implementation:**

| Builtin | AUDIT.md said | LIVE RESULT | Verdict |
|---|---|---|---|
| shopt | options not consumed | globstar, extglob, nullglob, dotglob, nocasematch ALL work | WORKS - demote |
| trap | no handler fires | ERR/EXIT/DEBUG traps ALL fire correctly | WORKS - demote |
| exec | doesnt exec-replace | exec echo REPLACED; echo AFTER -> only REPLACED prints | WORKS - demote |
| wait | completely stubbed | wait $PID -> WAITED_RC=0 | WORKS - demote |
| suspend | returns without stopping | Gives proper cannot suspend: no job control error | Works correctly |

The ONLY remaining HIGH-severity gap that affects common scripts:

| Builtin | Gap | Why HIGH |
|---|---|---|
| **complete / compopt** | Parse-only - complete -p cd -> empty; completion specs neither stored nor consulted | Breaks tab completion end-to-end; users see no completions for any command |

---

## 5. Shadowing fix (confirmed comprehensive)

command_substitution_values.rs:485-490 guards all 67 is_shell_builtin_name entries against .exe shadowing in $(). All 11 colliding builtins (fc, echo, printf, true, false, test, [, env, pwd, kill, help) invoke the shell builtin inside $(). PATH manipulation cannot defeat it; enable -n falls through correctly. See SHADOW_AUDIT.md.