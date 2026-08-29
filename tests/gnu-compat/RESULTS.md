# GNU Bash Compatibility Test Results

## Date: 2026-08-28

## Summary

| Metric | Result |
|--------|--------|
| **Pass rate** | **94.4% (34/36)** |
| Known bugs | 2 |
| Total tests | 36 |

## Test Results

### Passing (34)

- arith-assign, arith-expr, arithmetic-bool, array-basic
- brace-expand-assign, braces-char-range, braces-prefix, braces-range
- braces-simple, braces-step, case-basic, cmdsub-backtick
- cmdsub-dollar, exit-status, for-loop, function-basic
- glob-star, heredoc-basic, if-else, nested-cmdsub
- param-assign, param-default, param-length, param-prefix
- param-replace, param-suffix, pipe-basic, process-sub
- redirect-stdout, set-e, shopt-pipefail, string-compare
- while-loop, word-split

### Failing (2)

| Test | Expected | Got | Issue |
|------|----------|-----|-------|
| braces-nested | a1 a2 b1 b2 | a b 1 2 | Nested brace expansion |
| braces-triple | a1x a1y... | a b 1 2 x y | Triple nested brace expansion |

## Conclusion

rubash has **94.4% compatibility** with GNU Bash on Windows.
The only remaining bugs are in nested brace expansion.
