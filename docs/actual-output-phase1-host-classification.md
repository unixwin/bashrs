# Actual-Output Phase 1 Host and Runner Classification

Baseline checkpoint: 2c96b50
Raw evidence: target/issue-suites/results/bash-actual
Previous raw archive: target/issue-suites/results/bash-actual-pre-3cf9a7e

## Disposition

Phase 1 is complete for non-semantic classification. No Rust semantic owner was changed in this phase. The remaining semantic candidates are explicitly routed to later ordered phases.

### Status-pair evidence

- 12 cases with 127/0 are primarily runner or fixture contamination. Bash stderr contains missing test helpers or launcher artifacts such as recho: command not found, -c: command not found, and malformed exported-function fixture diagnostics, while Rubash stderr is empty. These must not be fixed in Rubash until a direct helper-complete invocation reproduces them.
- 5 cases with 124/0 are not one category. jobs contains deliberate sleep 300, sleep 350, and sleep 400 commands, so the 30 second per-test bound kills the fixture. ifs-posix generates 6856 tests and the Bash-side output is empty under the bound while Rubash reports all 6856 passed; this is a runner/performance bound, not proof of an IFS bug. read includes read -t and /dev/tty cases. redir includes host paths and permission fixtures. procsub remains a semantic candidate because its fixture includes the repeated fd/process-substitution bug loop.
- The 9/0 posix2 case is a semantic/parser candidate, not host noise: Bash reports 9 failed POSIX.2 checks while Rubash prints an eval syntax diagnostic and All tests passed. It is routed to Phase 4.
- The 127/0 list is: exportfunc, iquote, more-exp, nameref, new-exp, nquote1, nquote2, nquote3, nquote4, quote, shopt, and type. The common Bash-side helper/launcher evidence routes these to runner/fixture classification first; no Rust change is justified by the aggregate result.
- The 124/0 list is: ifs-posix, jobs, procsub, read, and redir. jobs is runner-timeout-owned. ifs-posix is runner-bound-owned. read and redir are mixed host-fixture plus semantic candidates and require bounded primitive probes in their later phases. procsub is routed to Phase 6 only after the ordered semantic phases, unless a direct minimal reproduction proves a process-substitution owner bug earlier.
- The 9/0 case is posix2 and is routed to Phase 4.

### Host and fixture contracts

The raw artifacts show these host-owned contracts:

- exportfunc, read, and redir exercise noninteractive /dev/tty or report its absence;
- redir expects Unix /etc/passwd, /tmp paths, Unix permission behavior, and external child fixtures;
- jobs exercises job control and signal delivery unavailable in the noninteractive Windows runner;
- several official tests use helper commands or launcher paths that are not present in the Winuxsh fixture environment;
- diagnostics emitted by external Windows commands remain localized host output.

These are documented differences, not candidates for shell semantic work in Phase 1.

## Phase 1 Gate

Satisfied:

- clean provenance checkpoint recorded as 2c96b50;
- raw current and prior result directories preserved;
- all non-semantic 124/0 and 127/0 families have a documented disposition;
- semantic candidates are separated from host/runner cases;
- no Rust semantic files changed during classification;
- cargo check and git diff --check passed before the provenance checkpoint;
- no residual Rubash, Bash, Cargo, or suite-runner process was present at the gate.

Next permitted phase: Phase 2, set-e and ERR suppression. Phase 3 and later remain blocked until Phase 2 closes.
