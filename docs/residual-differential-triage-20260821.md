# Residual Differential Triage

> Date: 2026-08-21
> Historical base: master f8e645e1
> Scope: historical bounded residual review for coproc, process substitution, part_080, and run-minimal.

This file preserves the 2026-08-21 interpretation ledger. Its `151/153`, `12/17`,
and `5/17` figures are pre-final-fix snapshots and must not be treated as the
current score. Raw command output remains under `target/issue-suites/results/`;
no expected output or core semantic owner was changed by that review.

## Current Evidence Superseding This Snapshot

The later focused evidence was collected at `a60a3faf` and remains valid on the
subsequent `b5e3f7b3` baseline unless a newer artifact says otherwise:

- `target/issue-suites/results/coproc-current-20260821/`: Bash and Rubash both
  completed the bridge-free coproc body with status 0 and empty stderr; `flop`,
  `REFLECT status 143`, and closed-fd diagnostics matched. Native versus virtual
  fd numbers, missing `xcase`/`/etc/passwd`, and localized Windows diagnostics
  are host or representation differences.
- `target/issue-suites/results/readonly-20260821-vredir/`: `vredir5` and `vredir7`
  matched Bash; `vredir8` differed only in the unavailable `/dev/tty` host
  diagnostic while fd recovery behavior matched.
- The old official `83/68` ledger is historical. An isolated raw ledger at
  `b5e3f7b3` is now available under
  `target/issue-suites/results/bash-ledger-refresh-b5e3f7b3/`: `83` rows,
  `13 PASS / 70 raw DIFF`. Its `results.tsv` is complete, but detached cleanup
  exited nonzero before `summary.txt` was written. The `70` rows require
  status-pair and fixture classification and are not 70 independent semantic
  defects.

The historical rows below remain useful for root-cause history, but any row marked
open must be revalidated against a dated artifact on the current commit before a
new Rust change is made.

A four-file Bash refresh at `b5e3f7b3` is recorded in
`target/issue-suites/results/bash-refresh-manual/`. Only `arith.tests` produced a
usable equal-status/equal-output result (`2/2`); `redir` and `coproc` used a
Winuxsh wrapper instead of GNU Bash, and `procsub` lacked `test-glue-functions`,
so those three entries are runner/fixture blocked. A Ksh representative probe
created `target/issue-suites/results/ksh-refresh-manual/manifest.txt` and its
input script but produced no status/stdout/stderr artifact; it is likewise a
runner-blocked attempt, not a Ksh semantic DIFF.

## Classification Summary

The table below is the final residual classification for this review. Bash entries
are control observations from the official runner or direct bounded probes;
Rubash entries are the focused test outputs. `Yes` means the difference is
attributable to host/fixture/harness behavior rather than a confirmed Rubash
semantic defect.

| Residual | GNU Bash result | Rubash result | Owner | Host/harness artifact? |
|---|---|---|---|---|
| Dynamic marker | Dynamic move control is `moved=alpha source=10 reused=10`, with closed-source status 0. | Current checkout also returns `moved=alpha source=10 reused=10` and `closed-status:0`; focused test passes 1/1. Historical `\x1c` output is covered by `bfadaeb8` and `63c2d7ea`. | Quoted-assignment marker decoding and dynamic array-field splitting; not an open fd-table defect. | No. Fixed and verified. |
| Side-file | Complete `p=>(...)` creates and cleans a side file; embedded `p=x>(...)y` starts the producer, creates the side file, and prints `/dev/fd/63`. | Rubash cleans the complete form, but the embedded form does not start the producer, does not create the side file, and prints a Windows temp path concatenated into the word. | Process-substitution embedded-assignment lifecycle/materialization in `external_setup.rs` and expansion/execution handoff. | No. Confirmed semantic candidate; probe artifact `target/ps-cleanup-probe/`. Keep the focused regression unchanged until Bash lifecycle behavior is implemented. |
| Coproc failures | Owner matrix matches Bash for descriptors, output read, wait status, and simple coproc probes. GNU sequential `cat -` returns `flop\n`, rc 0. | Focused slice is 12/17. Persistent-stderr test lacks asserted `done=127`; sequential `cat -` times out after 8s with empty stdout/stderr; three other failures are `\x1c` marker assertions. | Split ownership: variable expansion for markers; coproc external-child stdin/EOF materialization for sequential `cat -`; coproc status/CLI test contract for persistent stderr. | No for sequential timeout or markers. Persistent-stderr is unresolved, not safe to label harness noise. |
| run-minimal FAIL | GNU expected `/usr`, `/tmp`, and literal escaped tilde display. | Runner exits 0 but ledger records FAIL: Rubash/Windows emits `D:/usr`, `D:/tmp`, and differs in escaped-tilde display. | Upstream runner/fixture normalization; retain raw diff and do not change GNU expectations globally. | Yes. Known Windows host/path and harness classification difference. |

Supporting evidence: `docs/issue-suite-diff-analysis.md:2766-2798`,
`target/bash-upstream-tests/logs/run-minimal.log:49-59`, and the bounded
`part_080`/`cli_tests` commands below.

## Reproduction Commands

All commands were bounded. The focused commands used for this review were:

- `timeout 35s cargo test --test cli_tests c_command_coproc_child_inherits_persistent_stderr_to_stdout -- --nocapture`
- `timeout 35s cargo test --test cli_tests c_command_starts_cat_dash_coproc_after_waiting_for_previous_coproc -- --nocapture`
- `timeout 35s cargo test --test executor_tests command_chaining::part_080 -- --nocapture`

Observed results were 0/1 for the two individual coproc failures and 151/153
for part_080. The historical part_080 marker failure was `moved=\x1calpha source=10 reused=10`; current focused rerun at `7a722ae9` returns `moved=alpha source=10 reused=10` and `closed-status:0`, with the test passing 1/1. Existing fixes are `bfadaeb8` and `63c2d7ea`.

## Owner Queue

1. Variable storage/expansion owner: isolate the `\x1c` marker with a scalar
   assignment/read probe, then add a regression at the storage boundary. Do not
   conflate it with dynamic-fd move correctness.
2. External child/coprocess owner: trace inherited stdin, writer close, and
   endpoint retirement for the sequential `cat -` reproducer. Preserve a
   bounded timeout and archive stdout/stderr/rc for Bash and Rubash.
3. Process-substitution owner: fix the embedded-assignment lifecycle.
   `target/ps-cleanup-probe/` shows cleanup parity for the complete form, but
   Bash starts the embedded producer and creates the side file while Rubash does
   not. Preserve the focused regression and implement the producer/materialization
   handoff before changing expected output.
4. Runner/fixture owner: make `run-minimal` platform-aware for known Windows
   paths and escaped tilde display, while keeping the raw diff and exit status.
5. Test owner: resolve the persistent-stderr assertion only after recording the
   current exact Rubash output and GNU Bash control output; this may be a test
   contract issue, but is not yet safe to reclassify as fixture noise.

## Process Check

The bounded commands completed without a timeout at the harness level. A final
process check must be run by the parent agent immediately before integration,
because this delegated review does not own process cleanup outside its command
scope.
