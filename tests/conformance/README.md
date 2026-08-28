# Rubash Conformance Test Suite

Windows shell conformance tests where **rubash is the authoritative reference**.

## Philosophy

- Git Bash (MSYS2) is no longer the authoritative source for Windows bash behavior
- Rubash defines correct Windows shell semantics
- GNU Bash (Linux) tests are upstream reference, not acceptance gate
- Platform differences are marked, not forced to match

## Directory Structure

```
tests/conformance/
  core/           # 13 real bug regressions (from root cause analysis)
  windows/        # Windows-specific shell semantics
  compat/         # Compatibility markers (SKIP/BETTER/DIFF)
  reference/      # Rubash as authoritative source
  runner.sh       # Test runner
```

## Running

```bash
# Run all conformance tests
./tests/conformance/runner.sh

# Run specific category
./tests/conformance/runner.sh core
./tests/conformance/runner.sh windows

# Run against GNU Bash for comparison
RUBASH_COMPARE_BASH=1 ./tests/conformance/runner.sh
```

## Test Categories

### core/ - 13 Real Bugs
Regressions for the 13 genuine semantic differences found in root cause analysis.
These are the only tests that SHOULD fail against GNU Bash.

### windows/ - Windows Semantics
Tests for Windows-specific behavior: paths, devices, signals, coproc.

### compat/ - Compatibility Markers
- SKIP - Platform noise, not counted as bug (16 tests)
- BETTER - Rubash exceeds bash behavior (3 tests)
- DIFF - Real differences to track (13 tests)

## Why Not Just Use GNU Bash Tests?

The 83 upstream bash tests have:
- 16 platform noise (bash fails due to missing tools)
- 3 where rubash is better than bash
- 37 format differences (RC correct, output different)
- 13 real bugs

This suite focuses on what actually matters for Windows shell users.
