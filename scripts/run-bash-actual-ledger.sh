#!/usr/bin/env bash
set -u
set -o pipefail

repo="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
tests_dir="${BASH_LEDGER_TESTS_DIR:-$repo/third_party/bash/tests}"
out="${BASH_LEDGER_OUT:-$repo/target/issue-suites/results/bash-ledger-$(date -u +%Y%m%dT%H%M%SZ)}"
gnu="${BASH_LEDGER_GNU_BASH:-D:/Git/bin/bash.exe}"
rubash="${BASH_LEDGER_RUBASH:-$repo/target/debug/rubash.exe}"
timeout_bin="${BASH_LEDGER_TIMEOUT_BIN:-D:/Git/usr/bin/timeout.exe}"
timeout_seconds="${BASH_LEDGER_TIMEOUT_SECONDS:-30}"
test_filter="${BASH_LEDGER_TEST_FILTER:-}"

mkdir -p "$out/work/base/bash" "$out/work/base/rubash" "$out/work/results"
commit="$(git -C "$repo" rev-parse HEAD)"
printf 'commit=%s\nGNU_Bash=%s\nRubash=%s\ntimeout=%s\nnormalization=CRLF_to_LF\nfilter=%s\n' \
  "$commit" "$gnu" "$rubash" "$timeout_seconds" "$test_filter" > "$out/manifest.txt"
printf 'test\tstatus\tbash_rc\trubash_rc\n' > "$out/results.tsv"

cp -R "$tests_dir"/. "$out/work/base/bash/"
cp -R "$tests_dir"/. "$out/work/base/rubash/"
find "$out/work/base/bash" "$out/work/base/rubash" -type f -exec sed -i 's/\r$//' {} +
printf '#!/usr/bin/env bash\nexec "%s" "\$@"\n' "$gnu" > "$out/work/base/bash/bash"
printf '#!/usr/bin/env bash\nexec "%s" "\$@"\n' "$rubash" > "$out/work/base/rubash/bash"
chmod +x "$out/work/base/bash/bash" "$out/work/base/rubash/bash"

for src in "$tests_dir"/*.tests; do
  name="$(basename "$src" .tests)"
  if [[ -n "$test_filter" && ",${test_filter}," != *",${name},"* ]]; then
    continue
  fi
  result="$out/work/results/$name"
  mkdir -p "$result"
  (cd "$out/work/base/bash" && "$timeout_bin" "$timeout_seconds" "$gnu" "./$name.tests" >"$result/bash.stdout" 2>"$result/bash.stderr")
  brc=$?
  (cd "$out/work/base/rubash" && "$timeout_bin" "$timeout_seconds" "$rubash" "./$name.tests" >"$result/rubash.stdout" 2>"$result/rubash.stderr")
  rrc=$?
  status=PASS
  if [[ "$brc" != "$rrc" ]] || ! cmp -s "$result/bash.stdout" "$result/rubash.stdout"; then
    status=DIFF
  fi
  printf '%s\t%s\t%s\t%s\n' "$name" "$status" "$brc" "$rrc" >> "$out/results.tsv"
done

awk -F '\t' 'NR > 1 { total++; counts[$2]++ } END { printf "TOTAL=%d PASS=%d DIFF=%d\n", total, counts["PASS"] + 0, counts["DIFF"] + 0 }' \
  "$out/results.tsv" | tee "$out/summary.txt"
printf 'completed=yes\n' >> "$out/manifest.txt"
