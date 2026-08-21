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
- Windows logical-root/PWD: `13864d8a`.
- Coproc EOF, writer close, endpoint retirement, persistent stderr, and wait status: focused Bash/Rubash probes are parity; see `target/issue-suites/results/coproc-eof-current/`.

## Host/Fixture Classification

- `iquote`, `more-exp`, `nquote1..4`: repeated missing `recho` helper/output fixture contamination.
- `exportfunc`, `new-exp`: nested `THIS_SH`, `./bash`, `-c`, and `env -i` path assumptions.
- `glob`: mixed `recho`, permissions, locale, and Windows path behavior.
- Old `getopts 0/124`: current bounded run-getopts is 1/1 with byte-identical output and existing focused coverage.
- Coproc fd numbers, `/etc/passwd`, `xcase`, `/dev/tty`, `/usr`, `/tmp`, and `/proc` are host/product-contract or fixture evidence unless a direct semantic mismatch reproduces.

## Remaining Candidates

Needs direct bounded GNU Bash/Rubash probes before Rust changes:

- `arith 1/2`, `array 1/2`.
- `braces`, `comsub-posix`, `cond`, `mapfile`, `posixexp2`, `quotearray` with Bash rc=0 and Rubash rc=2.
- `builtins`, `comsub2`, `histexp` with Bash rc=2 and Rubash rc=0.
- `complete 2/1`, `rsh 0/1`, and `posix2 9/12`.
- `procsub/read/redir 124/0` and `trap/printf 2/124` need fixture-specific EOF/timeout reproduction; printf focused overflow behavior already passes.

Do not change expected output globally or treat a status pair alone as proof.

## Evidence

- `target/issue-suites/results/bash-ledger-current-a24c0379/`
- `target/issue-suites/results/bash-ledger-current-a24c0379/output-only-attribution.tsv`
- `target/issue-suites/results/coproc-eof-current/`
- `target/issue-suites/results/coproc-persistent-stderr-boundary/`
- `docs/residual-differential-triage-20260821.md`
- `target/issue-suites/results/cross-shell-current-20260822/`: BusyBox ash, Oil, and mksh unavailable; provider blockers, not Rubash DIFF.
