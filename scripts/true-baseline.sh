#!/usr/bin/env bash
# TRUE-baseline harness -- THE one measurement path for the 83 GNU suites.
#
# Usage (from Windows):
#   MSYS_NO_PATHCONV=1 wsl bash /mnt/d/repo/rubash/scripts/true-baseline.sh [suite...]
# With no arguments it runs all 83 suites; with arguments only those suites.
#
# Frozen methodology (do not hand-roll probes):
#   * tests are copied from third_party/bash/tests into bash-tests-rw with
#     CR stripped (the Windows checkout CRLFs them; GNU chokes on CR)
#   * GNU side: THIS_SH=/bin/bash so ${THIS_SH} sub-invocations run
#   * rubash side: no THIS_SH (auto-detects via current_exe),
#     __RUBASH_NO_UPSTREAM_SCRIPTS=1 so the real executor is measured
#   * both sides run with cwd=bash-tests-rw, PATH prefixed with it (recho /
#     zecho / run-* wrappers resolve), TMPDIR per suite, stdin </dev/null,
#     timeout -k 5 40
#   * the ledger diff counts stdout only (stderr is captured separately)
set -u

REPO=$(cd -- "$(dirname -- "${BASH_SOURCE[0]}")/.." && pwd)
BASE="$REPO/target/issue-suites/results/bash-tests-rw"
OUT="$REPO/target/issue-suites/results/true-baseline"
RUB="$REPO/target/debug/rubash.exe"
LOG="$REPO/target/issue-suites/results/true-baseline-ledger.log"
TESTS_SRC="$REPO/third_party/bash/tests"

# ---- sync: LF-normalized rw copies -----------------------------------------
mkdir -p "$BASE"
if [ ! -f "$BASE/recho" ] && [ -d "$TESTS_SRC" ]; then
  cp -r "$TESTS_SRC/." "$BASE/"
  find "$BASE" -type f \( -name "*.tests" -o -name "run-*" -o -name "*.right" -o -name "*.sub" \) \
    -exec sh -c 'tr -d "\r" < "$1" > "$1.lf" && mv "$1.lf" "$1"' _ {} \;
fi
# always re-normalize requested suites (the repo file is the source of truth)
sync_suite() {
  [ -f "$TESTS_SRC/$1.tests" ] || return 1
  tr -d "\r" < "$TESTS_SRC/$1.tests" > "$BASE/$1.tests"
}

# ---- suite list -------------------------------------------------------------
if [ $# -eq 0 ]; then
  SUITES=$(cd "$TESTS_SRC" && ls *.tests 2>/dev/null | sed "s/[.]tests$//")
else
  SUITES="$*"
fi

mkdir -p "$OUT"
: > "$LOG"
for name in $SUITES; do
  sync_suite "$name" || { echo "$name SKIP(no-source)" >> "$LOG"; continue; }
  w="$OUT/$name"; mkdir -p "$w/tmp"
  ( cd "$BASE" && PATH="$BASE:/usr/local/bin:/usr/bin:/bin" TMPDIR="$w/tmp" \
      THIS_SH=/bin/bash timeout -k 5 40 bash "./$name.tests" \
      > "$w/gnu.out" 2> "$w/gnu.err" ) < /dev/null
  echo $? > "$w/gnu.rc"
  ( cd "$BASE" && PATH="$BASE:/usr/local/bin:/usr/bin:/bin" TMPDIR="$w/tmp" \
      __RUBASH_NO_UPSTREAM_SCRIPTS=1 timeout -k 5 40 "$RUB" "./$name.tests" \
      > "$w/rb.out" 2> "$w/rb.err" ) < /dev/null
  echo $? > "$w/rb.rc"
  n=$(diff "$w/gnu.out" "$w/rb.out" 2>/dev/null | grep -c "^[<>]")
  echo "$name $n" >> "$LOG"
done
echo TRUE-DONE
