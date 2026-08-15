# Actual-Output Phase 2 Set-e and ERR Evidence

Checkpoint under test: 37780cf
Raw probes: target/issue-suites/results/native-bash-phase2/
GNU source: third_party/bash/execute_cmd.c and third_party/bash/trap.c
Rust owners: src/executor/embedded_mutations.rs, src/executor/pipeline_exec.rs, src/executor/pipeline_stages.rs, src/executor/ast_exec.rs

## Probe Gate

The bounded matrix contains 20 probes and compares status, stdout, and stderr against GNU Bash:

- simple && and || suppression;
- non-final brace and function pipeline stages;
- final compound pipeline stage;
- simple pipeline and pipefail;
- subshell, if, while, and ! contexts;
- ERR trap in simple, function, if, and pipeline contexts;
- command substitution with default suppression;
- command substitution with POSIX behavior;
- command substitution with inherit_errexit;
- command substitution failure status.

Result: 20/20 exact matches after the inherit_errexit fix.

## Root Cause Fixed

run_ast_command_substitution unconditionally wrapped the AST in with_errexit_suppressed. GNU keeps errexit active when POSIX mode or Bash shopt -s inherit_errexit applies. Rubash now queries the existing shopt state and preserves errexit for those modes.

Regression:

set -e; shopt -s inherit_errexit; x=$(false; echo sub); echo after

GNU and Rubash both exit with status 1 and produce no stdout.

## Existing Gates

- pipeline CLI focused tests: 18/18;
- arithmetic CLI focused tests: 12/12;
- wait executor tests: 19/19;
- trap executor tests: 24/24;
- command-chaining part 045 tests: 15/15;
- cargo check: passed;
- git diff --check: passed;
- no residual Rubash, Bash, Cargo, or suite-runner processes.

Phase 2 is complete. Phase 3 is the next permitted phase; parser, expansion, fd, and output phases remain locked.
