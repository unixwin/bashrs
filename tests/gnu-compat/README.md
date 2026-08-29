# GNU Bash Compatibility Test Suite

rubash vs GNU Bash (WSL) - .right file based testing

## Overview

This test suite verifies rubash compatibility against real GNU Bash running in WSL.
Unlike Git Bash (MSYS2), WSL provides a true GNU Bash environment.

## Versions

- **rubash**: 5.2.37(1)-release
- **GNU Bash (WSL)**: 5.2.21(1)-release

## Running Tests

```bash
# Run all tests
./tests/gnu-compat/run-test.sh

# Run specific test
./tests/gnu-compat/run-test.sh braces-nested

# List all tests
./tests/gnu-compat/run-test.sh list
```

## Test Results (Current)

- **Pass**: 34/36 (94.4%)
- **Fail**: 2/36 (known brace expansion bugs)

### Known Bugs

| Test | Issue |
|------|-------|
| braces-nested | `{a,b}{1,2}` not expanding |
| braces-triple | `{a,b}{1,2}{x,y}` not expanding |

## File Structure

```
tests/gnu-compat/
├── run-test.sh          # Test runner
├── README.md            # This file
├── tests/               # Test scripts (.sh)
├── rights/              # Expected output (.right)
└── work/                # Test artifacts
```

## Adding Tests

1. Create a test script in `tests/`
2. Generate .right file: `./run-test.sh <test-name>`
3. Verify the .right file is correct
4. Run tests to verify

## Why WSL Instead of Git Bash?

Git Bash (MSYS2) has:
- Missing tools (recho, zecho, printenv)
- Non-standard exit codes
- MSYS2-specific patches

WSL provides real GNU Bash, making it the authoritative reference.

## CI Integration

This suite is designed for manual use, not CI. For CI, use the conformance suite:
```bash
./tests/conformance/runner.sh
```
