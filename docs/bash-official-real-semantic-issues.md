# Bash Official Tests - Real Semantic Issues Analysis

## Excluded Categories

- **Missing Helper/Fixture** (8 tests): exportfunc, iquote, more-exp, new-exp, nquote1-4
- **Both Timeout** (2 tests): ifs-posix, jobs (environment issue)
- **Output-only diffs with same status** (34 tests): Need detailed output comparison

## Real Rubash Semantic Issues: 21 tests


### 1. Arithmetic error status (1 tests)

- **arith**: Bash=1, Rubash=2


### 2. Array operation error status (1 tests)

- **array**: Bash=1, Rubash=2


### 3. Brace expansion failure (1 tests)

- **braces**: Bash=0, Rubash=2


### 4. Builtin behavior mismatch (1 tests)

- **builtins**: Bash=2, Rubash=0


### 5. POSIX command substitution (1 tests)

- **comsub-posix**: Bash=0, Rubash=2


### 6. Command substitution variant (1 tests)

- **comsub2**: Bash=2, Rubash=0


### 7. Conditional expression (1 tests)

- **cond**: Bash=0, Rubash=2


### 8. Mapfile builtin (1 tests)

- **mapfile**: Bash=0, Rubash=2


### 9. POSIX parameter expansion (1 tests)

- **posixexp2**: Bash=0, Rubash=2


### 10. Quoted array assignment (1 tests)

- **quotearray**: Bash=0, Rubash=2


### 11. Alias command status (1 tests)

- **alias**: Bash=127, Rubash=1


### 12. Quote handling command (1 tests)

- **quote**: Bash=127, Rubash=2


### 13. Getopts hangs (1 tests)

- **getopts**: Bash=0, Rubash=124


### 14. Printf timeout (1 tests)

- **printf**: Bash=2, Rubash=124


### 15. Trap handling timeout (1 tests)

- **trap**: Bash=2, Rubash=124


### 16. Completion system (1 tests)

- **complete**: Bash=2, Rubash=1


### 17. Globbing behavior (1 tests)

- **glob**: Bash=0, Rubash=1


### 18. History expansion (1 tests)

- **histexp**: Bash=2, Rubash=0


### 19. POSIX mode differences (1 tests)

- **posix2**: Bash=9, Rubash=12


### 20. POSIX pipefail (1 tests)

- **posixpipe**: Bash=0, Rubash=1


### 21. Restricted shell feature gap (1 tests)

- **rsh**: Bash=0, Rubash=1




## Detailed Analysis of High Priority Issues

### 1. braces (Bash:0, Rubash:2)

**Root Cause**: Not brace expansion itself, but complex parameter expansion with quotes at line 59:
```bash
echo "${a#aaaa'$(aaaa'aaa)aaa\'}"
```

Rubash fails to parse this complex combination of:
- Parameter expansion (${a#...})
- Single quotes inside double quotes  
- Command substitution $(...)
- Escaped quotes

**GNU Source**: subst.c - extract_dollar_brace_string() handles quoted content within parameter expansion

**Fix Required**: Improve quote handling in parameter expansion parser to correctly handle nested quotes and command substitutions

**Estimated Effort**: Medium (requires parser changes)

---

### Next Steps

Given the complexity of analyzing all 21 issues individually, I recommend:

1. **Pick 1-2 simple issues** from the High Priority list that have clear GNU source correspondence
2. **Run individual test scripts** to see exact failures
3. **Trace through GNU C code** to understand expected behavior
4. **Implement fix in Rubash**
5. **Verify fix doesn't break other tests**

Would you like me to start with a specific issue from the High Priority list?

## Priority Ranking

### High Priority (Clear Bugs, Easy to Fix)
1. braces - Brace expansion returns 2 instead of 0
2. comsub-posix - POSIX command substitution handling
3. cond - Conditional expression evaluation
4. mapfile - Mapfile builtin behavior
5. posixexp2 - POSIX parameter expansion
6. quotearray - Quoted array assignment

### Medium Priority (Needs Investigation)
7. arith/array - Arithmetic/array error status codes
8. builtins/comsub2 - Builtin/command substitution behavior
9. getopts/printf/trap - Timeout/hanging issues
10. glob/complete - Globbing and completion differences

### Low Priority (Feature Gaps or Complex)
11. rsh - Restricted shell (feature gap)
12. posix2/posixpipe - POSIX mode differences
13. histexp - History expansion
14. alias/quote - Command not found vs wrong status
