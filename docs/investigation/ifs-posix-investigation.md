# ifs-posix Investigation: Subshell IFS Assignment Not Taking Effect in Read

## 1. Minimal Reproducer

**Script**: `target/issue-suites/results/probe/ifs-subshell-read.sh`

```bash
#!/bin/bash
ksh_arith=0
eval '((ksh_arith+=1))' 2>/dev/null
passed=0
failed=0

split()
{
    i=$1 s=$2 r=$3 S='' R=''
    for ifs in ': ' ' :'
    do  
        IFS=$ifs
        set x $i
        shift
        IFS=' '
        g="[$#]"
        while :
        do  case $# in
            0) break ;;
            esac
            g="$g($1)"
            shift
        done
        g=$(export ifs; echo "$i" | ( IFS=$ifs; read x y; echo "($x)($y)" ))
        case $g in
        "$r")  case $ksh_arith in 1) ((passed+=1));; *) passed=$(expr $passed + 1);; esac ;;
        *)     case $ksh_arith in 1) ((failed+=1));; *) failed=$(expr $failed + 1);; esac ;;
        esac
    done
}

str='a b c'
IFS=' '
set x $str
shift
f1=$1; shift
f2=$1; shift
f3=$1; shift

for d0 in '' ' '
do
    for d1 in ':' ' :' ': ' ' : '
    do
        case ' ' in
        $f1$d1|$d1$f2) continue ;;
        esac
        for d2 in ' ' ':' ' :' ': ' ' : '
        do
            case $f2$d2 in ' ') continue ;; esac
            case ' ' in $f2$d2|$d2$f3) continue ;; esac
            for d3 in '' ' ' ':' ' :' ': ' ' : '
            do
                case $f3$d3 in
                '')  split "$d0$f1$d1$f2$d2$f3$d3" "[2]($f1)($f2)" "($f1)($f2)" ;;
                ' ') ;;
                *)   x=$f2$d2$f3$d3
                     x=${x#' '}
                     x=${x%' '}
                     split "$d0$f1$d1$f2$d2$f3$d3" "[3]($f1)($f2)($f3)" "($f1)($x)"
                     ;;
                esac
            done
        done
    done
done

echo "# tests $((passed+failed)) passed $passed failed $failed"
```

**GNU Bash 5.2.21 output** (byte-exact):
```
# tests 480 passed 480 failed 0
```

**Rubash output** (byte-exact):
```
# tests 480 passed 0 failed 480
```

## 2. GNU Source Evidence

**File**: `third_party/bash/builtins/read.def`

The GNU read builtin retrieves IFS via `getifs()` (defined in `third_party/bash/subst.c:12368`), which reads the current shell variable `IFS` from the variable table. In GNU Bash, when `IFS=$ifs` is executed as a standalone assignment inside a compound subshell `( IFS=$ifs; read x y; ... )`, the assignment permanently modifies the IFS variable in the subshell's environment (it is NOT treated as a temporary/prefix assignment because the semicolon separates it from the `read` command). The subsequent `read` then calls `getifs()` and retrieves the correct value.

Key lines in `read.def`:
- L419-430: `ifs_chars = getifs();` — reads current IFS
- L969: `alist = list_string(input_string, ifs_chars, 0);` — splits using IFS
- L998-1001: leading/trailing IFS whitespace stripping

## 3. Rust Owner

**Primary file**: `src/executor/command_dispatch.rs` (L1-38)
**Secondary file**: `src/executor/temporary_assignments.rs` (L1-28)
**Tertiary file**: `src/executor/pipeline_stages.rs` (L41-99) — `execute_compound_pipeline_stage`

**What it does differently**: In `execute_materialized_command` (`command_dispatch.rs` L4-38), rubash applies ALL command-level assignments as temporary assignments (L13: `apply_temporary_assignments`) and then restores them after command execution (L32: `restore_temporary_assignments`), UNLESS `keeps_temporary_assignments` returns true. For a standalone assignment like `IFS=$ifs` (no following command word), `keeps_temporary_assignments` returns false (L133-134: returns false when `cmd.words.first()` is None). This means the IFS assignment is applied, the command body executes (no-op for a bare assignment), and then the old IFS value is restored — all before the next command in the list (`read x y`) runs.

In the compound pipeline stage (`pipeline_stages.rs` L41-99), the subshell is created via `command_substitution_executor()` (L48) which clones the parent's env_vars. The compound subshell body `( IFS=$ifs; read x y; echo "($x)($y)" )` is executed as a list of commands via `execute_ast`. Each command goes through `execute_materialized_command`, so the standalone `IFS=$ifs` assignment is treated as temporary and immediately restored.

This differs from GNU Bash where standalone assignments in a command list permanently modify the shell variable. GNU distinguishes:
- **Prefix assignments** (`VAR=val cmd`): temporary, visible only during cmd execution  
- **Standalone assignments** (`VAR=val; cmd`): permanent in the current shell scope

Rubash conflates these by making ALL assignments in `execute_materialized_command` temporary unless the command is a special builtin (export, declare, etc.).

## 4. Root Cause

The root cause is that rubash's `execute_materialized_command` in `src/executor/command_dispatch.rs` treats standalone variable assignments (assignments without a following command word) as temporary assignments that are immediately restored after the no-op command body executes. In GNU Bash, standalone assignments in a command list are permanent: they modify the shell variable for all subsequent commands in the same scope. The distinction between prefix assignments (`VAR=val cmd`) and standalone assignments (`VAR=val; cmd`) is fundamental to POSIX shell semantics. Rubash's `keeps_temporary_assignments` function only preserves assignments for special builtins (export, declare, typeset, readonly), but the real issue is the opposite direction: standalone assignments should be PERMANENT, not temporary. The temporary-restore cycle in `execute_materialized_command` (L13 apply, L23 execute, L32 restore) incorrectly undoes the assignment before the next command in the list sees it.

## 5. Proposed Source-Consistent Fix

**File**: `src/executor/command_dispatch.rs`, function `execute_materialized_command`

**Before** (L9-33):
```rust
let keep_temporary_assignments = self.keeps_temporary_assignments(cmd);
// ...
let temporary_assignments = self.apply_temporary_assignments(&cmd.assignments);
// ...
let result = self.execute_prepared_command(cmd);
// ...
if !keep_temporary_assignments {
    self.restore_temporary_assignments(temporary_assignments);
}
```

**After**:
```rust
let keep_temporary_assignments = self.keeps_temporary_assignments(cmd);
// Standalone assignments (no command word) are permanent in POSIX;
// only prefix assignments before a command are temporary.
let is_standalone_assignment = cmd.words.is_empty() && !cmd.assignments.is_empty();
let temporary_assignments = if is_standalone_assignment {
    // Apply permanently: no restore needed
    self.apply_permanent_assignments(&cmd.assignments);
    Vec::new()
} else {
    self.apply_temporary_assignments(&cmd.assignments)
};
// ...
let result = self.execute_prepared_command(cmd);
// ...
if !keep_temporary_assignments && !temporary_assignments.is_empty() {
    self.restore_temporary_assignments(temporary_assignments);
}
```

A new helper `apply_permanent_assignments` would apply each assignment directly to `env_vars` without recording restoration state (or simply call `apply_shell_assignment` for each). The key semantic change: when a CommandNode has assignments but NO command words, the assignments are permanent, matching GNU Bash's behavior for standalone assignments in a command list.

**Overlap note**: This change touches the same `command_dispatch.rs` and `temporary_assignments.rs` files modified by the accepted IFS positional state repair (t10/t13). The captain should serialize this fix after verifying the existing repair is stable.

## 6. Verification Plan

**Focused Rust test**: Add a test in `tests/executor_command_chaining/` (e.g., `part_081.rs`) that exercises:
1. A function containing `IFS=': '; set x $i; shift; g=$(echo "$i" | ( IFS=$ifs; read x y; echo "($x)($y)" ))`
2. Called from a for-loop context with nested loops
3. Asserts that the read result matches GNU Bash behavior

**run-83.sh check**:
```
MSYS_NO_PATHCONV=1 wsl bash tests/gnu-compat/run-83.sh check ifs-posix
```

The check should show PASS ifs-posix (currently TIMEOUT because the upstream script bridge doesn't fire from the clean test directory, and native execution takes >15s). After the fix, the native execution should produce the same output as GNU Bash for the full 6856-test suite.
