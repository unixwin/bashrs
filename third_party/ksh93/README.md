# Vendored ksh93 regression suite

Verbatim copy of ksh93/ksh `src/cmd/ksh93/tests` (`58` items) plus the
`bin/shtests` wrapper, captured for the ksh93 differential issue (#42).

Upstream: https://github.com/ksh93/ksh (clone of 2026-09-07, master).

Run: `bash third_party/ksh93/shtests SHELL=<shell>` from a POSIX shell,
or the repo runner `scripts/run-ksh93-difftest.sh` which drives the GNU
bash / rubash sides with per-test timeouts. ksh93-specific semantics that
GNU bash does not define are classified, not bugs.
