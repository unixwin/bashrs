# Phase 3 Arithmetic Triage

Current checkpoint: d150188
Raw artifact: target/issue-suites/results/native-bash-phase3-arith/
GNU owner: third_party/bash/expr.c
Rubash semantic owners: src/executor/arithmetic/, src/executor/command_execute.rs, src/executor/command_prepare.rs

## Important Finding

The official arith.tests result is intercepted by the upstream bridge. In src/executor/upstream_scripts/handlers_b.rs, execute_upstream_arith_script matches a script name ending in arith.tests, emits the canned ARITH_TEST_OUTPUT, sets ARITH_TEST_DONE, and forces exit_code to 0.

Therefore the official slice result bash=1 and rubash=0, together with the large canned-output diff, is not evidence that the native arithmetic evaluator returned the wrong status. Renaming the identical fixture to copy.tests or prefix.sh bypasses the bridge and executes the native owner; the native fixture returns rc=1 for the final arithmetic errors.

This is a bridge-owned artifact. The bridge must not be removed until the native arithmetic fixture is sufficiently covered and its output/status is verified against GNU, in accordance with the repository migration rule.

## Native Evidence

Focused native probes match GNU for:

- conditional false-branch assignment and side effects;
- invalid octal/base diagnostics;
- nounset status 127;
- division by zero;
- malformed arithmetic command status;
- invalid arithmetic expansion status;
- invalid array-subscript assignment status;
- multi-command arithmetic error status.

The 20 Phase 2 set-e probes remain green and are not part of this phase.

## New Native Findings

GNU Bash treats arithmetic expansion errors differently depending on the entry mode. When the same fixture is read from a script file, errors such as invalid bases and division by zero report status 1 and the script continues; the same echo arithmetic error followed by echo after passed with -c exits before after. The implementation now preserves this script-mode behavior while retaining the existing -c fatal regression and the existing errexit gate.

The focused regression arithmetic_assignment_error_continues_without_errexit covers the first native mismatch: an assignment expansion with an attempted assignment to a non-variable returns status 1, does not install the assignment, and permits the next command in script mode. The arithmetic-focused suite is 13/13 green.

The full native fixture was rerun as copy.tests to bypass the bridge. Bash and Rubash both return rc=1. Replacing line 191, ((echo abc; echo def;); echo ghi), with : allows both executions to reach the end; this isolates that stop to parser/AST handling and routes it to the ordered parser phase. With that line bypassed, one evaluator mismatch remains: GNU suppresses output for A=4 + ; echo arithmetic expansion, while Rubash emits 20; this remains a Phase 3 arithmetic evaluator candidate.

## Remaining Phase 3 Work

1. Run the native arithmetic fixture under a non-bridge filename with stdout and stderr captured separately.
2. Compare native output/status against GNU and split diagnostics-channel differences from evaluator differences.
3. Add focused regressions only for a reproducible native semantic mismatch.
4. Decide whether the bridge can be narrowed, refreshed, or removed only after native coverage and semantic-map evidence.

Phase 3 remains in progress. Phase 4 and later remain locked.
