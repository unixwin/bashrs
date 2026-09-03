# Vendored BusyBox ash test suite

Verbatim copy of the BusyBox ash TestSuite, used as a runnable comparison
gate for rubash (BusyBox ash vs rubash difftest). Do NOT edit the files
under `ash_test/`; they are upstream-verbatim. Durable interpretation of
results lives in run reports / docs, raw artifacts under
`target/busybox-results/`.

## Upstream source

- Project: BusyBox <https://busybox.net/>
- Version: **1.36.1** (official release tarball)
- URL: <https://busybox.net/downloads/busybox-1.36.1.tar.bz2>
- Tarball sha256: `b8cc24c9574d809e7279c3be349795c5d5ceb6fdf19ca709f80cde50e47de314`
- Vendored from tarball path: `shell/ash_test/` -> this directory
  (`third_party/busybox/tests/ash_test/`), byte-for-byte, LF line endings
  (verified: 0 files containing CR).
- License: GPLv2 — `../LICENSE` copied verbatim from the same tarball.
- Vendored: 2026-09-03, busybox-ash difftest integration task.

## Contents

- `ash_test/ash-*/` — 17 test directories (alias, arith, comm, getopts,
  glob, heredoc, invert, misc, parsing, psubst, quoting, read, redir,
  signals, standalone, vars, z_slow) with **335** `*.tests` items and
  336 `*.right` expected-output files (the one extra `ash-arith/arith-for.right`
  has no `*.tests` sibling upstream; every `*.tests` has a `.right`).
- `ash_test/run-all` — upstream harness. Expects a busybox binary linked
  as `./ash`, a `.config`, and compiles `recho`/`zecho`/`printenv` from
  the vendored `*.c` (these are verbatim copies of GNU Bash's test
  helpers by Chet Ramey). Not used directly by the rubash gate; kept for
  upstream parity.
- `ash_test/{recho,zecho,printenv}.c` — helper program sources, compiled
  per-run into the writable scratch copy by the runner.

## How rubash runs this suite

Runner: `scripts/run-busybox-ash-difftest.sh` (WSL side):

    wsl bash /mnt/d/repo/rubash/scripts/run-busybox-ash-difftest.sh [ash-dir ...]

Each item runs under BOTH BusyBox ash (WSL, reference side) and
`target/debug/rubash.exe` (via WSL interop), with a mandatory per-item
timeout (default 10s; `BUSYBOX_TEST_TIMEOUT` to override). The known P0
hang `ash-heredoc/heredoc_huge.tests` must surface as TIMEOUT, never as a
runner hang. Every child runs with `< /dev/null`: a test script's `read`
can otherwise consume the driver's loop input stream (documented
stdin-consumption leak, docs/bash-compat-issues.md family B).

Note: the historical "143-item / 17-directory" run referenced in
docs/bash-compat-issues.md (#26) covered a subset; the full vendored
upstream suite is 335 items across the same 17 directories.

## Hygiene rules

- The vendored tree is read-only; the runner stages a writable copy under
  `target/busybox-results/ash_test-rw/` (tests write `$0.tmp` files etc.).
- Raw per-item artifacts land under `target/busybox-results/<RUN_ID>/`.
- Keep this tree LF; CRLF breaks WSL-side execution.
