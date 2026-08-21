# Residual Differential Triage

> Date: 2026-08-21
> Base: master f8e645e1
> Scope: bounded residual review for coproc, process substitution, part_080, and run-minimal.

This is an interpretation ledger. Raw command output remains under
`target/issue-suites/results/`; no expected output or core semantic owner was
changed during this review.

## Classification Summary

The table below is the final residual classification for this review. Bash entries
are control observations from the official runner or direct bounded probes;
Rubash entries are the focused test outputs. `Yes` means the difference is
attributable to host/fixture/harness behavior rather than a confirmed Rubash
semantic defect.

| Residual | GNU Bash result | Rubash result | Owner | Host/harness artifact? |
|---|---|---|---|---|
| Dynamic marker | Dynamic move control is `moved=alpha source=10 reused=10`, with closed-source status 0. | `part_080` is 151/153; actual `moved=\x1calpha source=10 reused=10`, status otherwise correct. | Variable storage/expansion and marker decoding; not fd-table move ownership. | No. Confirmed Rubash marker leak. |
| Side-file | Independent process-substitution owner probes match Bash; no Bash side-file failure is recorded for this exact embedded-assignment assertion. | `test_embedded_assignment_output_process_substitution_keeps_surrounding_text` fails because the side file exists; this is the second `part_080` failure. | Process-substitution producer cleanup/lifetime, likely `src/executor/process_substitution.rs` and deferred child cleanup. | Not yet. Candidate Rubash cleanup defect; requires exact Bash control and side-file contents/status before coding. |
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
for part_080. The part_080 stderr identifies the dynamic marker failure as
`moved=\x1calpha source=10 reused=10`; no source file was edited.

## Owner Queue

1. Variable storage/expansion owner: isolate the `\x1c` marker with a scalar
   assignment/read probe, then add a regression at the storage boundary. Do not
   conflate it with dynamic-fd move correctness.
2. External child/coprocess owner: trace inherited stdin, writer close, and
   endpoint retirement for the sequential `cat -` reproducer. Preserve a
   bounded timeout and archive stdout/stderr/rc for Bash and Rubash.
3. Process-substitution owner: inspect the side-file producer and cleanup status
   for the embedded assignment case. Establish whether the file is a deliberate
   producer artifact or an uncollected temporary before changing code.
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
