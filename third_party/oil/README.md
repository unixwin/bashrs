# Vendored Oil spec corpus

Oil spec test files (*.test.sh) plus the sh_spec harness from the
oils-for-unix source tree, captured for the Oil spec runner issue (#40).

Upstream: https://github.com/oilshell/oil (shallow clone of 2026-09-07).

Run (Python 3):
  python3 third_party/oil/test/sh_spec.py third_party/oil/spec/*.test \
      --osh-... with THIS_SH / PATH fixture setup recorded per #40's
      acceptance notes.
The Oil-specific semantics that GNU bash does not define are classified
as Oil-only evidence, not Bash-compatible bugs. Bash-reference parity
runs must set up spec/bin helpers, TMPDIR, and a healthy GNU bash side
before counting DIFF.
