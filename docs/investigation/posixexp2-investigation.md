# posixexp2 Investigation: Cases 8, 9, 11, 12, 28, 29, 37, 39

## 1. Minimal Reproducers

Each failing case was extracted verbatim from third_party/bash/tests/posixexp2.tests.
Individual scripts saved under target/issue-suites/results/probe/posixexp2-case{N}.sh.

### Case 37 (simplest - backslash-space in unquoted default)

WSL GNU:  `37 <a b> <x> <c d> .`
Rubash:   `37 <a> <b> <x> <c> <d> .`

### Case 39 (suffix removal with quotes)

WSL GNU:  `39 x' / x' .`
Rubash:   `39 x / x' .`

### Case 8 (escaped double-quote inside IFS+ default)

WSL GNU:  `8 ""z}`
Rubash:   `8 \"z}`

### Case 9 (escaped double-quote+brace inside IFS+ default)

WSL GNU:  `9 "}"z`
Rubash:   `9 \}"z`

### Case 11 (nested cmdsubst with single-quoted default)

WSL GNU:  `11 ''z}`
Rubash:   `11 }z`

### Case 12 (nested cmdsubst, unquoted)

WSL GNU:  `12 }z`
Rubash:   `12 z}`

### Case 28 (complex multi-level quoting)

WSL GNU:  `28 'x ~ x''x}"x}" #`
Rubash:   `28 x ~ x}x}x #`

### Case 29 (complex brace expansion)

WSL GNU:  `29 <foo> <abx{ {{> <{}b> <c> <d{}> <bar> <}> <baz> .`
Rubash:   `29 <foo> <abx{ {{ {}b> <c> <d{> <}> <bar> <}> <baz> .`

## 2. GNU Source Evidence

### Group A: Case 37 - Backslash-space in unquoted default

GNU source: subst.c, extract_dollar_brace_string() lines 1825-2017.

When extracting the default word from `$\{v-a\\ b\}`, GNU's extract_dollar_brace_string
encounters backslash followed by space. The pass_character mechanism (lines 1853-1858, 1861-1866)
consumes both characters. The extracted word then goes through parameter_brace_expand_rhs (line 7966)
which calls expand_string_for_rhs. GNU preserves the word as a single unit during word splitting.

Rubash owner: src/executor/parameter_words.rs expand_parameter_word() lines 4-13, and
src/executor/expand_braced_ops.rs expand_braced_operator_or_array_parameter() lines 97-103.

The Rust expand_parameter_word calls expand_embedded_parameters then unescape_remaining_shell_escapes.
The backslash is consumed but the resulting space triggers IFS field splitting. GNU treats
the backslash-escaped space as part of the word during word splitting.

### Group B: Cases 8/9 - Escaped double-quote in IFS+ alternate word

GNU source: subst.c param_expand() at lines 10348-10382 for the + operator, then
parameter_brace_expand_rhs() lines 7966-8093. Lines 7982-7988 apply string_extract_double_quoted
with SX_STRIPDQ to the alternate word in double-quoted context.

For the alternate word backslash-quote in double-quoted context: the backslash escapes the
double-quote, producing just the quote character. This is handled by the two-stage pipeline:
extract_dollar_brace_string extracts the word, then parameter_brace_expand_rhs processes
it with context-aware double-quote handling.

Rubash owner: src/executor/parameter_words.rs expand_quoted_parameter_word_mut() lines 326-336.

The expand_embedded_parameters_mut_with_context function passes both characters through as
literal in DoubleQuoted context. Then expand_quoted_parameter_operator_word processes the
result: decode_double_quotes_in_quoted_parameter_word treats the double-quote as opening a
quoted region with no close, consuming it. Only the backslash survives. The result is
backslash instead of the expected double-quote.

### Group C: Cases 11/12 - Nested cmdsubst with single-quoted default

GNU source: parse.y parse_matched_pair() lines 4035-4036 (Austin Group Interp 221),
subst.c extract_dollar_brace_string() lines 1951-1963.

In POSIX mode inside double quotes, single quotes inside ${...} are literal unless
dolbrace_state is Quote or Quote2. For ${IFS+'}'z}, after the + operator the state
is DOLBRACE_OP, so the single quote is literal. The brace closes at }, producing
the word single-quote. parameter_brace_expand_rhs then processes this in double-quoted
context via string_extract_double_quoted, preserving the single-quote as a literal.

Rubash owner: src/lexer/dolbrace.rs scan_braced_parameter() lines 121-137 correctly
implements Interp 221. The issue is in how the extracted single-quote character is
subsequently expanded - it is being treated as a quote delimiter rather than literal.

### Group D: Case 28 - Complex multi-level quoting

This case has 4+ levels of nested quoting inside ${...} inside double quotes.
GNU tracks this through extract_dollar_brace_string's state machine. The Rust code's
flat string processing in expand_embedded_parameters_ordered_mut does not maintain
equivalent quote-nesting depth.

### Group E: Case 29 - Complex brace/quote field boundaries

The default word contains nested braces, embedded quotes, and variable references.
The field boundary difference (<abx{ {{> <{}b> vs <abx{ {{ {}b>) indicates different
brace/quote context tracking during word splitting in embedded_mutations.rs and
braces.rs.

### Group F: Case 39 - Suffix removal quote preservation

GNU source: subst.c param_expand() -> parameter_brace_remove_pattern() ->
get_pattern_string(). The pattern *'a'* has single quotes treated as quoting
characters (pattern becomes *a*). The result x' retains the trailing single
quote as a literal character in the expansion result.

Rubash owner: src/lexer/quotes.rs remove_shell_quotes() lines 4-93, specifically
lines 45-57. After parameter expansion produces the value x', the unquoted word
expansion path applies remove_shell_quotes which treats the trailing single-quote
as an opening quote character. When no closing quote is found, the quote is
silently dropped, producing x instead of x'.

The critical call site: src/executor/command_prepare.rs lines 548-552 applies
remove_shell_quotes to expanded text when word_contains_brace_group is true.
This incorrectly applies lexer-level quote removal to runtime expansion results.

## 3. Root Causes

Three distinct root causes:

**RC-1 (Cases 8, 9, 11, 12, 28):** The parameter expansion word processor does not
correctly handle backslash-escaped characters and quote nesting inside the
alternate/default word of ${...} in double-quoted context. GNU uses a two-stage
pipeline (extract with backslash-aware scanning, then process with context-aware
quote handling). Rubash's pipeline does not replicate this.

**RC-2 (Case 37):** The unquoted parameter expansion default word loses backslash-escape
context before word splitting. GNU preserves the word as a single unit; Rubash allows
the space to trigger IFS splitting.

**RC-3 (Case 39):** remove_shell_quotes is incorrectly applied to parameter expansion
results. The trailing single-quote in x' is a literal character, not a quote delimiter.

## 4. Proposed Source-Consistent Fixes

### Fix for RC-3 (Case 39)

**File:** src/executor/command_prepare.rs lines 548-552

**Before:**
```rust
let expanded = self.expand_word_mut_with_context(word, context);
let expanded = if word_contains_brace_group(word) && !word.starts_with('\x1d') {
    crate::lexer::remove_shell_quotes(&expanded)
} else {
    expanded
};
```

**After:**
```rust
let expanded = self.expand_word_mut_with_context(word, context);
// Do not apply remove_shell_quotes to expansion results. Quote removal
// applies to original lexer tokens, not runtime expansion values.
let expanded = if word_contains_brace_group(word) && !word.starts_with('\x1d') {
    expanded
} else {
    expanded
};
```

**Why:** GNU word expansion produces a WORD descriptor with quoting metadata. Quote
removal only applies to original word tokens. Rubash post-expansion remove_shell_quotes
incorrectly strips quote characters from expansion results.

**Risk:** May affect command_chaining::part_063 (known 5 failures).

### Fix for RC-1 (Cases 8, 9, 11, 12, 28)

**File:** src/executor/parameter_words.rs expand_quoted_parameter_word_mut() and
src/executor/embedded_mutations.rs expand_embedded_parameters_ordered_mut()

The alternate word processing needs to handle backslash-escape sequences in the
parameter expansion word. When the alternate word is backslash-quote in DoubleQuoted
context, the backslash should escape the quote, producing just the quote character.

This requires either:
(a) Modifying expand_embedded_parameters_ordered_mut to handle backslash-quote as
    an escape sequence in DoubleQuoted context, OR
(b) Pre-processing the alternate word to resolve backslash escapes before passing
    to expand_embedded_parameters_mut_with_context.

### Fix for RC-2 (Case 37)

**File:** src/executor/expand_braced_ops.rs and src/executor/parameter_words.rs

The expand_parameter_word function needs to preserve context that the word came
from a parameter expansion default. Spaces that were originally backslash-escaped
should not trigger word splitting. This requires carrying quoting metadata through
the expansion pipeline.

## 5. Verification Plan

### Focused Rust Tests
```
cargo test --test parameter_expansion_tests posixexp2
cargo test --test command_chaining_tests part_063
```

### Bounded run-83.sh Check
```
MSYS_NO_PATHCONV=1 wsl bash tests/gnu-compat/run-83.sh check posixexp2
```

### Expected Impact
- Cases 8, 9: Fixed by RC-1
- Cases 11, 12, 28: Fixed by RC-1
- Case 37: Fixed by RC-2
- Case 39: Fixed by RC-3

### Risk Assessment
- RC-3 is safest (removes incorrectly applied post-processing)
- RC-1 requires careful modification of parameter expansion pipeline
- RC-2 requires fundamental word splitting context changes

### Cross-Family Overlap
- RC-3 fix may affect command_chaining::part_063
- Captain should serialize RC-3 fix with part_063 verification
