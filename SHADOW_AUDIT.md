# Rubash Command-Substitution Shadowing Audit (t1 follow-up)

Scope: verify the command_substitution_values.rs fix is comprehensive across ALL
execution paths and all 67 names in is_shell_builtin_name, plus Windows edge cases.

---

## 1. The fix (command_substitution_values.rs:485-490)

```rust
if is_shell_builtin_name(first_word)
    && !crate::builtins::enable::is_disabled(&self.env_vars, first_word) {
    return Ok(None);
}
let Some(program) = find_user_command(&stdio.expanded_words[0], &self.env_vars) else { ... }
```
Guard checks the NAME before PATH lookup, AND honors `enable -n` (disabled builtins
fall through to external). Correct placement.

---

## 2. Full call-site audit of find_user_command / is_shell_builtin_name

Every site that could shadow a builtin was checked. Result: **no unguarded sites remain**.

| File:line | Context | Guarded? | Verdict |
|---|---|---|---|
| command_substitution_values.rs:485-490 | run_external_command_substitution — THE FIX | is_shell_builtin_name + is_disabled | FIXED |
| command_substitution_values.rs:119 | filter fallback (sort/sed/tr/head/grep/wc/tail/uniq) | Only reached for external filter names (none are builtins) | safe by construction |
| pipeline_exec.rs:656-659 | native-all-external pipeline (yes/head/wc) | if is_shell_builtin_name { return None } | safe |
| pipeline_exec.rs:817-820 | native pipeline stage | if is_shell_builtin_name { return None } | safe |
| pipeline_exec.rs:1115 | `sed` arm fallback | Only reached when word == "sed" (not a builtin) | safe |
| pipeline_exec.rs:1166 | `tr` arm fallback | Only reached when word == "tr" (not a builtin) | safe |
| pipeline_exec.rs:1175-1178 | default pipeline dispatch | builtin stage first, external fallback | safe |
| pipeline_stages.rs:175 | execute_builtin_pipeline_stage | if !is_shell_builtin_name { return None } | safe |
| pipeline_stages.rs:241 | execute_external_pipeline_stage_inner | Only reached after builtin stage returned None | safe by construction |
| external_inner.rs:40 | `env` builtin running a command | env deliberately runs external; env IS the builtin | safe |
| external_inner.rs:396 | execute_external_command | Caller dispatches builtin FIRST before reaching here | safe by construction |
| lookup_paths.rs:34 | command_paths (type/which display) | Informational only; never executes | safe |
| lookup_paths.rs:39 | is_enabled_shell_builtin_name | is_shell_builtin_name && !is_disabled | safe |
| sudo_builtin.rs:47 | sudo looking up target | sudo deliberately runs externals | safe |
| path.rs:79,143,500,501 | bash/pwsh/powershell helper lookups | System-tool lookups, not user command dispatch | safe |

NO OTHER unguarded execution path was found beyond the one that was fixed.

---

## 3. Windows collision table — the 11 builtins with a real .exe on PATH

These are the ONLY builtin names that have a same-named executable reachable on a
typical Windows PATH (system32 + winuxcmd/usr/bin). All 67 is_shell_builtin_name
entries were checked; only these 11 collide. Every one was tested live in `$(...)`.

| Builtin | Has .exe on PATH? | $(builtin) works? | Other issues |
|---|---|---|---|
| fc | YES — C:/WINDOWS/system32/fc.exe (the original blocker) | YES — `$(fc -ln -1)` returns builtin history output ("cmd_b"), even with system32 first in PATH | Root cause fixed |
| echo | YES — winuxcmd/usr/bin/echo.exe | YES — `$(echo -n hello)` -> "hello" (no trailing newline, builtin-only behavior) | none |
| printf | YES — winuxcmd/usr/bin/printf.exe | YES — `$(printf '%s=%s\n' k v)` -> "k=v" | none |
| true | YES — winuxcmd/usr/bin/true.exe | YES — `$(true); echo $?` -> 0 | none |
| false | YES — winuxcmd/usr/bin/false.exe | YES — `$(false); echo $?` -> 1 | none |
| test | YES — winuxcmd/usr/bin/test.exe | YES — `$(test 1 -eq 1); echo $?` -> 0 | none |
| [ | YES — winuxcmd/usr/bin/[.exe | YES — `$( [ 2 -gt 1 ] ); echo $?` -> 0 | none |
| env | YES — winuxcmd/usr/bin/env.exe | YES — `$(env)` prints env vars (124 lines) | none |
| pwd | YES — winuxcmd/usr/bin/pwd.exe | YES — `$(pwd)` returns PWD | none |
| kill | YES — winuxcmd/usr/bin/kill.exe | YES — `$(kill -l 0)` -> "EXIT" (builtin signal table) | none |
| help | YES — C:/WINDOWS/system32/help.exe | YES — `$(help cd)` runs (RC=0); builtin help topic lookup | note: `which help` reports system32 help.exe (cosmetic; see below) |

Non-colliding builtins (56 names) — alias, bg, bind, break, builtin, caller, cd,
command, compgen, complete, compopt, continue, declare, dirs, disown, enable, eval,
exec, exit, export, fc, fg, getopts, hash, history, jobs, let, local, logout,
mapfile, popd, pushd, read, readarray, readonly, return, set, setopt, shift,
shopt, source, suspend, times, trap, type, typeset, ulimit, umask, unalias,
unset, unsetopt, wait, sudo — have NO same-named .exe on PATH, so there was never
a shadowing risk for them in the first place.

---

## 4. Windows-specific edge cases

| Edge case | Result | Verdict |
|---|---|---|
| `which fc` | Reports `C:/WINDOWS/system32/fc.exe` | EXPECTED — `which` is an EXTERNAL (winuxcmd/usr/bin/which.exe), not a rubash builtin; it only does PATH lookup. `type fc` correctly reports "fc is a shell builtin". This is the standard which-vs-type semantic difference, not a shadowing bug. |
| `$PATH` prepended with system32, then `$(fc ...)` | `$(fc -ln -1)` STILL returns builtin output ("cmd_b") | FIX IS NAME-BASED, not PATH-based. PATH ordering cannot defeat it. CONFIRMED safe. |
| `enable -n fc` then `$(fc)` | `type fc` -> "fc.exe"; `$(fc)` falls through to external fc.exe (RC reflects external) | CORRECT fallthrough behavior — disabled builtins intentionally become external. The is_disabled check in the fix handles this. |
| Alias pointing to builtin inside `$()` (non-interactive) | Alias not expanded inside `$()` | EXPECTED — aliases are disabled in non-interactive shells (matches GNU bash). Not a shadowing issue; requires interactive shell or `shopt -s expand_aliases`. |
| Nested `$()` with builtins | `$(echo $(printf '%s' inner))` -> "inner" | Works correctly. |
| Pipeline with builtin inside `$()` | `$(echo a b c \| tr 'a-z' 'A-Z')` -> "A B C" | Works correctly. |

---

## 5. Summary

- The single-site fix in `run_external_command_substitution` is SUFFICIENT. Every
  other `find_user_command` call site is either guarded by `is_shell_builtin_name`
  or unreachable for builtin names by construction (caller dispatches builtin first).
- All 11 colliding builtins now correctly invoke the shell builtin inside `$()`;
  the fix is name-based so it cannot be defeated by PATH manipulation.
- `enable -n` correctly disables the builtin and falls through to the external — the
  `is_disabled` clause in the fix is exercised and working.
- `which` reporting the .exe for builtins is a cosmetic which-vs-type semantic
  difference (which is external), not a shadowing regression.
- No additional execution paths with similar bypass risk were found.
