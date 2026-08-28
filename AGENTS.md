# Rubash Agent Entry Point

Before making compatibility changes, read:

1. `docs/gnu-bash-compatibility-implementation-plan.md`
2. `docs/issue-suite-diff-analysis.md`
3. `docs/bash-compat-issues.md`
4. `docs/bash-source-map.md`

Key rules:

- **CRITICAL: Always use GNU bash (D:/Git/bin/bash.exe) for comparisons, NOT winuxsh shim.** The winuxsh shim is at PATH bash and is an older version with different behavior. Run comparisons as: `D:/Git/bin/bash.exe third_party/bash/tests/X.tests` NOT: `bash third_party/bash/tests/X.tests`
- Fix by root-cause subsystem, not by individual expected-output lines.
- Keep raw suite artifacts under `target/issue-suites/results/`; keep durable
  interpretation in `docs/`.
- Do not run full suites unbounded. Use per-test, per-file, or per-directory
  timeouts where possible.
- Do not remove `src/executor/upstream_scripts*` until the corresponding
  behavior is covered by real semantics and suite slices are green.
- Treat `src/builtins/kill.rs` and `src/input/readline/kill.rs` as different
  semantic owners: Bash builtin vs readline editing.
- Check for stuck `rubash.exe` / `bash.exe` / suite runner processes before
  finishing a testing turn.

## External bashdb

`bashdb` is available as an external Bash-script debugger for Rubash
compatibility work. Use it to debug shell-script behavior running under
`target/debug/rubash.exe`: source mapping, stepping, function stacks, traps,
`eval`, arrays, options, and other Bash semantics. Do not treat bashdb as a
Rust-source debugger; use Rust tooling, logs, instrumentation, and focused tests
for `src/**/*.rs` internals.

The verified local fixture is `target/bashdb-clean/bashdb-generated` with its
library directory at `target/bashdb-clean`. Keep bashdb external and clean: do
not patch bashdb as the product fix. Temporary instrumentation in
`target/bashdb-clean` is allowed for diagnosis only, and must be reverted before
finishing.

Quick smoke test:

```sh
export TERM=xterm DARK_BG=0
printf 'list\nstep\nnext\nwhere\ncontinue\nwhere\nquit\n' | \
  target/debug/rubash.exe target/bashdb-clean/bashdb-generated --no-highlight target/bashdb-probe-target.sh
```

A passing smoke test exits `0`, has empty stderr, lists the target script, steps
into `foo`, prints a stack with `where`, and continues through `42` / `done`.
For setup details, launcher/libdir terminology, and fresh-checkout usage, see
`docs/bashdb-debugging-rubash.md`. Treat full bashdb command coverage as a
development target: each failing bashdb command should normally drive a Rubash
root-cause compatibility fix, not a bashdb patch.
