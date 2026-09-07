# Vendored mksh check.pl

Verbatim copy of mksh's check.pl regression harness (and tests/ where
present), captured for the mksh runner portability issue (#41).

Upstream: https://github.com/MirBSD/mksh (shallow clone of 2026-09-07).

Run under a POSIX shell with a stable working directory:
  perl third_party/mksh/check.pl -s <shell-under-test>
History showed native Perl/cwd boundary failures on Windows; run it
from WSL with the shell under test reachable through a wrapper.
mksh-specific semantics that GNU bash does not define are recorded as
host/fixture evidence, not Bash-compatible bugs.
