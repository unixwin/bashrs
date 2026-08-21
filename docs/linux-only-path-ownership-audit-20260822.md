# Linux-only Path Representation Ownership Audit

> Date: 2026-08-22
> Scope: `/dev/fd`, `/dev/tty`, `/proc`, `/usr`, and `/tmp` differences observed while comparing GNU Bash, Rubash, and the Winuxsh/WinuxCmd contract.
> Policy: evidence and ownership only. No Rubash engine change, no global expected-output change, and no Linux path emulation is justified by this audit.

## Contract Boundary

Rubash owns Bash-visible semantics: parsing, expansion, redirection order, virtual fd identity and lifetime, process-substitution/coprocess state, and diagnostics/status rules. Winuxsh owns shell hosting and registration of compatibility namespaces. WinuxCmd owns Windows filesystem, process, handle, pipe, and virtual-path provider primitives. A Linux pathname appearing in a GNU test is not by itself a requirement to create a real Linux pathname on Windows.

The Windows-first release contract explicitly defers complete Linux support. Differences must therefore be classified as one of:

- **Host-owned**: a Windows capability or provider is absent, or the host controls the native representation.
- **Product contract**: Rubash must preserve a Bash-observable semantic, but its Windows representation may differ and must be documented.
- **Engine fd semantic bug**: the observable fd/process-substitution behavior itself differs after host representation is accounted for.

## Evidence Matrix

| Linux-only surface | GNU Bash contract | Current evidence | Classification | Owner / action |
|---|---|---|---|---|
| `/dev/fd/N` in process substitution | Endpoint is usable as a word operand; producer starts, endpoint remains valid, and cleanup follows lifetime. Native fd numbers are not portable. | The historical `target/ps-cleanup-probe/embedded/` mismatch (Windows temp-path leak and `side_exists=no`) was fixed by `5f77bf2c` (`fix: run embedded assignment process substitutions`). | **Engine fd semantic bug: fixed** | Rubash fix is committed and covered by the focused command-chaining regression. Keep the raw probe as historical evidence; do not normalize the temp path in expected output. |
| `/dev/fd/N` numeric identity | Bash exposes an fd-like endpoint, but native numbers are implementation-dependent. Contract is endpoint identity/lifetime, not Linux numbers. | Coproc evidence records distinct Rubash virtual descriptors (10/11 versus Bash 63/60); body, wait, and closed-fd diagnostics match in `target/issue-suites/results/coproc-current-20260821/`. | **Product contract** | Rubash owns fd identity/lifetime; WinuxCmd materializes child handles. Do not force GNU fd numbers. |
| `/dev/tty` | GNU opens the controlling terminal when one exists; noninteractive environments may report unavailable-device errors. | `readonly-20260821-vredir/vredir8`: stdout matches (`redir2`); only unavailable-device wording differs (`No such device or address` versus localized Windows `os error 2`). | **Host-owned** | No Rubash change. Current evidence does not establish a Windows host defect. |
| `/proc/self/fd` and `/proc/<pid>/fd` | Linux exposes a live per-process descriptor namespace with readlink/enumeration semantics. | `docs/proc-pid-fd-compatibility-plan.md` says Rubash has only selected special cases, no namespace, and no WinuxCmd provider API in this workspace. | **Host boundary blocker**, not engine bug | Do not create a fake directory or guessed target strings in Rubash. Winuxsh registration plus WinuxCmd read-only provider is required first. |
| `/usr` and `/tmp` literals | GNU/Linux tests print literal roots and use them as fixture paths. | `run-minimal.log` records `D:/usr` and `D:/tmp` versus GNU `/usr` and `/tmp`; `docs/windows-logical-root.md` defines the Windows logical-root mapping. | **Product contract / harness representation** | Keep the Windows logical-root contract. Classify path prefixes in the runner; do not change GNU expected output globally. |
| `/usr/bin/cat` and absolute command paths | Shell resolves and launches according to platform command/path contract; diagnostics include platform paths. | Ledger records the `/usr/bin/cat` execution-boundary fix at `docs/issue-suite-diff-analysis.md:2636`; remaining path-prefix differences are Windows representation. | **Product contract**, unless lookup/status differs | Rubash owns command semantics; WinuxCmd owns native lookup. Reproduce a status/lookup mismatch directly before changing either layer. |

## Explicit Non-findings

1. Native Bash fd numbers versus Rubash virtual numbers are not an fd bug when direction, lifetime, read/write behavior, cleanup, and status match.
2. Missing noninteractive `/dev/tty` is not repaired by mapping it to `NUL`; that changes controlling-terminal semantics.
3. `/proc` must not be implemented by a physical directory, environment-variable guesses, or preformatted Rubash output.
4. `D:/usr` and `D:/tmp` are expected Windows representations under the logical-root contract, not grounds for rewriting GNU expected files.
5. Embedded process substitution was a real Rubash owner item, but it is closed by `5f77bf2c` (`fix: run embedded assignment process substitutions`). The historical probe remains evidence of the original lifecycle/representation bug; it is no longer an open audit finding.

## Durable References

- Raw `/dev/tty`: `target/issue-suites/results/readonly-20260821-vredir/`.
- Historical raw embedded process substitution: `target/ps-cleanup-probe/embedded/`; fixed by commit `5f77bf2c`.
- Raw coprocess comparison: `target/issue-suites/results/coproc-current-20260821/`.
- `/proc` provider design: `docs/proc-pid-fd-compatibility-plan.md`.
- Logical-root contract: `docs/windows-logical-root.md`.
- Residual queue: `docs/residual-differential-triage-20260821.md`.
- Canonical map: `docs/semantic-ownership.tsv`.

## Gate Before Any Code Change

For a future change, archive a dated Bash/Rubash/host artifact with command, stdout, stderr, status, timeout, and process cleanup. The embedded process-substitution lifecycle finding in this audit is closed by `5f77bf2c`; reopen it only with a new reproduction. Modify Rubash only for a newly reproduced semantic mismatch. Modify Winuxsh/WinuxCmd only after a Windows product contract is stated and the host boundary fails it. Never satisfy Linux-only display strings by changing the Rubash engine or global expected output.
