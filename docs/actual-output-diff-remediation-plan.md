# Actual-Output DIFF Remediation Plan

Status: ordered execution plan
Baseline checkpoint: 2c96b50
Source: target/issue-suites/results/bash-actual/results.tsv

Rule: execute phases strictly in order. A later phase cannot start while an earlier phase has an unresolved gate.

## Current Baseline

83 tests were run with a per-test 30 second timeout:

- PASS: 12
- DIFF 0/0: 38
- DIFF 0/1: 1
- DIFF 0/2: 1
- DIFF 1/0: 2
- DIFF 2/0: 10
- DIFF 2/2: 1
- DIFF 9/0: 1
- DIFF 124/0: 5
- DIFF 127/0: 12
- Total DIFF: 71

These pairs are triage counts, not a compatibility score. The runner compares status and stdout, while stderr is evidence only. Host failures and diagnostics can therefore look like shell failures.

## Ordered Phases

### Phase 0: Freeze provenance

Verify a clean tree, record checkpoint, binary paths, GNU Bash path, timeout, host, and timestamp. Archive each raw result directory and never overwrite an archive. Check for residual rubash, bash, cargo, and suite-runner processes.

Gate: provenance and process state are recorded.

### Phase 1: Classify runner and host differences

Handle all 124/0, 127/0, 9/0, /dev/tty, /etc/passwd, xcase, external-child lookup, and localized-diagnostic cases. Reproduce each outside the suite wrapper and record stdout, stderr, status, timeout, and ownership: semantic, host, fixture, or runner.

No Rust semantic owner changes are allowed in this phase.

Gate: every non-semantic status pair has a documented disposition and raw artifact.

### Phase 2: Fix set-e and ERR suppression

Owners: ast_exec.rs, pipeline_exec.rs, pipeline_stages.rs, command-substitution execution, and focused tests. Compare GNU execute_cmd.c and trap.c. Cover simple commands, pipelines, non-final/final brace and function stages, &&, ||, !, functions, subshells, command substitution, inherit_errexit, ERR, and errtrace.

Gate: each mismatch has a minimal regression and all existing pipeline, wait, and trap tests remain green.

### Phase 3: Fix arithmetic evaluator and status

Owners: executor/arithmetic, command_execute.rs, command_prepare.rs, arithmetic_aliases.rs, and arithmetic regressions. Compare GNU expr.c and execute_cmd.c. Cover nounset 127, invalid base/octal, division by zero, malformed constants, conditional assignment branches, assignment-to-non-variable, let, arithmetic command, assignment expansion, and arithmetic expansion.

Gate: GNU status, stdout, stderr, error token, and variable side effects match.

### Phase 4: Fix parser and POSIX expansion

Owners: parser, parameter_errors.rs, and POSIX expansion owners. Compare GNU parse.y, subst.c, posixexp.c, and posixpat.c. Cover malformed compound commands, brace termination, unmatched parameters, valid newline forms, POSIX pattern expansion, status 2, and source-token diagnostics.

Gate: malformed forms match GNU without rejecting valid forms.

### Phase 5: Fix arrays, IFS, RHS, and command substitution expansion

Owners: arrays.rs, command_prepare.rs, parameter_ops.rs, expand_braced_replacement.rs, and command-substitution value owners. Cover quoted/unquoted $@, array at/star, sparse and empty elements, IFS boundaries, escaped separators, empty RHS, command substitution in RHS, and assignment contexts.

Gate: field count, empty fields, separators, side effects, and status match GNU.

### Phase 6: Fix fd, process substitution, jobs, and traps

Enter only after Phases 1-5 pass. Cover dynamic fd allocation/move/close, varredir_close, ordered redirects, heredoc precedence, persistent process substitution cleanup, wait repeated status, wait -n/-p, coproc EOF/retirement, and trap ordering.

Gate: existing fd, process-substitution, coproc, wait, and trap focused tests stay green.

### Phase 7: Fix output and diagnostic contracts

Only fix a renderer or diagnostic after Phase 1 proves it is not host-owned. Cover source token preservation, spacing, semicolons, stdout/stderr channel, and status.

Gate: every change has a GNU probe and focused regression.

### Phase 8: Re-run and checkpoint

Run the full bounded suite from a clean tree, create a new dated result directory, recompute status pairs, update the ledger and issue-suite analysis, run cargo check, focused tests, semantic-map validation, git diff --check, and process checks. Review the owner diff, then commit and push.

## Non-Negotiable Rules

- No parallel semantic edits across phases.
- No Rust change based only on one DIFF line.
- Every fix requires GNU source evidence, a minimal probe, and a regression.
- Host/runner failures are documented, not disguised as shell behavior.
- A failed gate stops the plan at that phase.
- The next phase starts only after the current phase is explicitly complete.

## Execution Status

- Phase 0: complete at checkpoint 2c96b50.
- Phase 1: complete for host/runner classification; evidence is in `docs/actual-output-phase1-host-classification.md`.
- Phase 2: complete; evidence is in `docs/actual-output-phase2-set-e-evidence.md`.
- Next permitted phase: Phase 3, arithmetic evaluator and status.
- Phases 4-8 remain blocked until Phase 3 closes.
