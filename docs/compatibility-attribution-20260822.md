# Compatibility Attribution Ledger

> Current Bash actual rerun: commit `a24c0379`; raw artifact: `target/issue-suites/results/bash-ledger-current-a24c0379/`.

## Current Counts

The authoritative `results.tsv` contains 83 test rows: 13 PASS and 70 raw DIFF. The runner summary says TOTAL=82 because its summary loop skips the real test named `test`; counts here are recomputed from all rows.

Status pairs:

`0/0=41, 0/1=3, 0/2=6, 0/124=1, 1/2=2, 2/0=3, 2/1=1, 2/2=5, 2/124=2, 9/12=1, 124/0=3, 124/124=2, 127/0=6, 127/1=1, 127/2=3, 127/127=3`.

A status pair or raw DIFF is not an independent bug.

## Fixed Engine Families

- Dynamic marker/array-field split: `bfadaeb8`, `63c2d7ea`.
- Embedded assignment process substitution: `5f77bf2c`, `f173f927`; focused family 56/56.
- Nameref array element and positional slice offset zero: `f5e63f99`.
- printf integer overflow saturation/warning/status: `9d1a8033`; printf tests 30/30.
- Extglob parse-time rc=2: `a24c0379`; parser tests 15/15.
- Arithmetic conditional error-token rendering: `a80e07d8`; focused regression passes (`+=2` matches Bash).
- `enable -d` invalid builtin status/usage: `4a9e8d46`; focused regression passes with rc=2.
- Whitespace-braced substitution rejection (`${ printf ...; }`): `1e1b367b`; focused executor regression passes with rc=1.
- Windows logical-root/PWD: `13864d8a`.
- Coproc EOF, writer close, endpoint retirement, persistent stderr, and wait status: focused Bash/Rubash probes are parity; see `target/issue-suites/results/coproc-eof-current/`.

## Host/Fixture Classification

- `iquote`, `more-exp`, `nquote1..4`: repeated missing `recho` helper/output fixture contamination.
- `exportfunc`, `new-exp`: nested `THIS_SH`, `./bash`, `-c`, and `env -i` path assumptions.
- `glob`: mixed `recho`, permissions, locale, and Windows path behavior.
- Old `getopts 0/124`: current bounded run-getopts is 1/1 with byte-identical output and existing focused coverage.
- Coproc fd numbers, `/etc/passwd`, `xcase`, `/dev/tty`, `/usr`, `/tmp`, and `/proc` are host/product-contract or fixture evidence unless a direct semantic mismatch reproduces.
- `procsub`, `read`, `redir`, and `trap` bounded probes are Bash/Rubash parity; raw `printf` width probe times out under the same 8-second wrapper for both engines. See `target/issue-suites/results/status-candidates-20220822/classification.tsv`.
- Official `rsh 0/1` is contaminated by the Rubash upstream canned bridge (`handlers_a.rs`/`inline_b.rs`). Independent `-c` evidence confirms `set -r` is not implemented in the Rubash engine: `set -r` is invalid, `$-` lacks `r`, and absolute commands/`cd` are not restricted. See `target/issue-suites/results/rsh-probe-20220822/`.

## Remaining Candidates

Remaining candidates after bounded probes:

- `array 1/2`: real empty/`*`/negative subscript diagnostic and error-propagation mismatch; current uncommitted patch still fails the full focused regression with status 1 and is not integrated.
- `comsub-posix`: valid output mismatch (`abc )` versus `abc eof )`), artifact-backed and not yet fixed.
- `posixexp2`: valid rc/output mismatch (Bash rc=0 and `1 }z`; Rubash rc=2), not yet fixed.
- `quotearray`: associative quoted subscript/value mismatch and arithmetic diagnostic divergence, not yet fixed.
- `comsub2` now rejects the unsupported `${ command; }` form like Bash via `1e1b367b`; classify this as a compatibility guard/feature boundary, not proof that Rubash lacked a Bash-supported command-substitution feature. `builtins` is fixed by `4a9e8d46`; `arith` is fixed by `a80e07d8`.
- `rsh 0/1` remains bridge/stream-policy evidence; independent `set -r` is an engine feature gap without an integrated owner implementation.
- `braces` is invalid due malformed nested quoting; `cond` and `mapfile` are valid parity; `histexp` is interactive/host-sensitive.
- `complete` and `posix2` remain unclassified and need a new focused artifact.

Do not change expected output globally or treat a status pair alone as proof.

## Evidence

- `target/issue-suites/results/bash-ledger-current-a24c0379/`
- `target/issue-suites/results/bash-ledger-current-a24c0379/output-only-attribution.tsv`
- `target/issue-suites/results/coproc-eof-current/`
- `target/issue-suites/results/coproc-persistent-stderr-boundary/`
- `docs/residual-differential-triage-20260821.md`
- `target/issue-suites/results/cross-shell-current-20260822/`: BusyBox ash, Oil, and mksh unavailable; provider blockers, not Rubash DIFF.
- `target/issue-suites/results/ksh-current-20260822/`: ksh93, ksh, and mksh unavailable; bounded probe returned provider-blocked 127 without executing a ksh script.
