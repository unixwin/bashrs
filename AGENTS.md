# Rubash Agent Entry Point

Before making compatibility changes, read:

1. `docs/gnu-bash-compatibility-implementation-plan.md`
2. `docs/issue-suite-diff-analysis.md`
3. `docs/bash-compat-issues.md`
4. `docs/bash-source-map.md`

Key rules:

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
