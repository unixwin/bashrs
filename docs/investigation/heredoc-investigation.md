# Heredoc Collection Boundary Investigation

## 1. Minimal Reproducer

The smallest script that shows the bug (comsub-posix heredoc boundary):

```bash
#!/bin/bash
echo $(cat <<eof
here doc with )
eof
)
```

**Byte-exact WSL GNU Bash 5.2.21 output:**

```
here doc with )
```

**Byte-exact rubash output:**

```
here doc with
target/issue-suites/results/probe/comsub-heredoc-paren.sh: line 4: eof: command not found
```

WSL exits 0 with the heredoc body `here doc with )` correctly piped to `cat`. Rubash truncates the body to `here doc with` (losing the `)` that was part of the heredoc), treats the delimiter line `eof` as a command, and exits 127.

**Secondary reproducer (EOF) delimiter — heredoc3 pattern):**

```bash
#!/bin/bash
unbalanced=$(cat <<EOF
this paren ) is not a problem
EOF)
echo $unbalanced
```

WSL: `this paren ) is not a problem` (with warning about unterminated here-doc)
Rubash: `this paren is not a problem` + `EOF: command not found`

**Tertiary reproducer (heredoc7 pattern — heredoc inside command substitution):**

```bash
#!/bin/bash
echo $(cat << EOF)
foo
bar
EOF
after
```

WSL: `foo bar` (correct — heredoc body is "foo\nbar", cat gets "foo bar")
Rubash: empty line + `foo: command not found` + `bar: command not found` + `EOF: command not found` + `after: command not found`

All three reproducer scripts are saved under `target/issue-suites/results/probe/`.

---

## 2. GNU Source Evidence

### GNU Bash heredoc reading: `make_here_document` in `make_cmd.c:512-639`

GNU Bash reads the heredoc body **after** the command is fully parsed. The grammar actions in `parse.y` call `push_heredoc()` (lines 627/634/641/648/655/662) when a `<<` redirect is parsed. Then `gather_here_documents()` (line 3120) calls `make_here_document()`, which reads lines via `read_secondary_line(delim_unquoted)` (line 569) directly from the input stream:

```c
// make_cmd.c:569
while (full_line = read_secondary_line (delim_unquoted))
{
    line = full_line;
    // ...
    if (STREQN (line, redir_word, redir_len) && line[redir_len] == '\n')
        break;
    // append line to document
}
```

The critical point: `make_here_document` reads raw lines from the input stream. The heredoc body content is **never fed back to the parser's parenthesis counter**. Characters like `)` inside the heredoc body are just bytes in the line — they do not affect command-substitution depth tracking.

### GNU Bash command substitution: `parse.y` grammar

When the parser encounters `$(`, it enters a recursive `parse_and_execute` call via `command_substitute` (`subst.c:7143`). Inside the sub-parser, `<<` tokens trigger `push_heredoc`. The heredoc body is collected by `gather_here_documents()` at natural newline-termination points (line 3650-3651 in `read_token_word`, and grammar actions at lines 1266/1344/1362/1377). The heredoc body lines are consumed from the input stream **before** the parser ever sees a `)` that might close the command substitution.

### Key invariants in GNU Bash:

1. **Heredoc body is opaque to the parser**: `make_here_document` reads raw lines via `read_secondary_line`. The parser's parenthesis/quote state is frozen during heredoc reading.
2. **Command substitution ends at the matching `)`**: The `)` that closes `$()` is parsed as a token **after** all pending heredocs have been collected.
3. **`gather_here_documents` is called at newline boundaries**: Lines 3650-3651, 1266, 1344, 1362, 1377 — whenever the parser reaches a newline with pending heredocs, it collects them before continuing.

---

## 3. Rust Owner

### Primary owner: `src/lexer/continuation.rs` — `skip_parenthesized_unit` (lines 162-198)

This function is called by `has_unclosed_command_substitution` (line 349) to quickly check if a $() is balanced. It scans characters between `(` and `)`, tracking quote state but **completely ignoring heredoc operators (`<<`)**:

```rust
// continuation.rs:162-198
fn skip_parenthesized_unit(chars: &[char], open: usize) -> Option<usize> {
    let mut depth = 0usize;
    let mut index = open;
    let mut single = false;
    let mut double = false;
    while index < chars.len() {
        let ch = chars[index];
        if single { /* skip single-quoted */ }
        if double { /* skip double-quoted */ }
        match ch {
            '\''' => single = true,
            '\"' => double = true,
            '(' => depth += 1,
            ')' => {
                depth = depth.saturating_sub(1);
                if depth == 0 {
                    return Some(index + 1);  // returns as balanced
                }
            }
            _ => {}
        }
        index += 1;
    }
    None
}
```

When the input is `echo $(cat <<eof\nhere doc with )`, this function starts at `(`, increments depth to 1, then scans through `cat <<eof\nhere doc with `, and finally finds the `)` in `here doc with )` which brings depth to 0. It returns `Some(...)`, falsely concluding the command substitution is balanced.

### Secondary owner: `src/lexer/mod.rs` — `tokenize_with_heredocs` (lines 58-182)

The line-accumulation loop (lines 72-168) depends on `has_unclosed_command_substitution` to decide when to stop accumulating lines. When that function returns false prematurely (because `skip_parenthesized_unit` was fooled), the loop finalizes the logical_line and tokenizes it. The heredoc body collector (lines 116-164) then reads the remaining lines, consuming the delimiter but leaving the command-substitution closing `)` as a separate orphan token.

### How they interact:

1. Line `echo $(cat <<eof` arrives → `has_unclosed_command_substitution` → true (depth=1, no `)` yet) → continue accumulating
2. Line `here doc with )` arrives → `has_unclosed_command_substitution` calls `skip_parenthesized_unit` → finds `)` → returns false
3. Logical_line `echo $(cat <<eof\nhere doc with )` is finalized and tokenized
4. Heredoc delimiters detected: `<<eof` with `allow_closing_paren=true`
5. Body loop reads next line `eof` → matches delimiter → empty body
6. Remaining line `)` → standalone token → "eof: command not found" or syntax error

---

## 4. Root Cause

In GNU Bash, the parser separates two concerns that Rubash's `tokenize_with_heredocs` conflates: (a) determining when a command-substitution `$()` is syntactically complete, and (b) collecting heredoc body lines that belong to a redirect inside that command substitution. GNU Bash's `make_here_document` reads heredoc body lines directly from the input stream via `read_secondary_line`, making them invisible to the parser's parenthesis counter. A `)` inside a heredoc body is consumed by the heredoc reader and never seen by the parser, so it cannot prematurely close the command substitution.

Rubash's `skip_parenthesized_unit` in `continuation.rs` scans characters linearly for balanced parentheses without recognizing `<<` as a heredoc operator whose body should be skipped. When the logical_line contains $(cat <<word\n<body with )>), the function encounters the `)` inside the heredoc body and returns `Some(...)`, falsely reporting the command substitution as balanced. This causes `tokenize_with_heredocs` to finalize the logical_line before the heredoc body has been collected, and the subsequent heredoc body collection consumes the delimiter as an empty body while leaving the command-substitution closing `)` as an orphan.

---

## 5. Proposed Source-Consistent Fix

### Fix location: `src/lexer/continuation.rs` — `skip_parenthesized_unit` (lines 162-198)

**Captain-exclusive file** — do not edit directly; describe the change for captain application.

The fix adds heredoc body skipping inside `skip_parenthesized_unit`, mirroring the existing logic that `has_unclosed_command_substitution` already uses for depth > 0 at lines 410-413:

```rust
// existing code in has_unclosed_command_substitution (lines 410-413):
if depth > 0 && ch == '<' && chars.get(index + 1) == Some(&'<') {
    index = skip_heredoc_in_chars(&chars, index);
    continue;
}
```

**Before (skip_parenthesized_unit, lines 183-195):**

```rust
match ch {
    '\''' => single = true,
    '\"' => double = true,
    '(' => depth += 1,
    ')' => {
        depth = depth.saturating_sub(1);
        if depth == 0 {
            return Some(index + 1);
        }
    }
    _ => {}
}
index += 1;
```

**After:**

```rust
// Skip heredoc bodies: <<word consumes lines until the delimiter,
// making the body opaque to parenthesis balancing.
// This mirrors the existing heredoc skip in has_unclosed_command_substitution.
if !single && !double && ch == '<' && chars.get(index + 1) == Some('<')
    && chars.get(index + 2) != Some(&'<')
{
    index = super::heredoc_scan::skip_heredoc_in_chars(chars, index);
    continue;
}
match ch {
    '\''' => single = true,
    '\"' => double = true,
    '(' => depth += 1,
    ')' => {
        depth = depth.saturating_sub(1);
        if depth == 0 {
            return Some(index + 1);
        }
    }
    _ => {}
}
index += 1;
```

**Why this matches C behavior**: GNU Bash's `make_here_document` reads heredoc body lines from the input stream using `read_secondary_line`, which is entirely separate from the parser's token-reading path. The heredoc body content (including any `)` characters) never reaches the parser's parenthesis counter. The proposed fix achieves the same effect by having `skip_parenthesized_unit` skip over heredoc bodies using the existing `skip_heredoc_in_chars` helper, preventing `)` characters inside heredoc bodies from being counted as command-substitution closers.

**Overlap note**: The `has_unclosed_command_substitution` function already has heredoc-skipping logic at lines 410-413 for the case where `skip_parenthesized_unit` returns `None` (unbalanced). After this fix, `skip_parenthesized_unit` will handle the heredoc case itself, so the lines 410-413 code becomes a redundant safety net. Both paths use the same `skip_heredoc_in_chars` helper, so there is no behavioral conflict.

### Corrective finding: mod.rs is required for same-line header close

The earlier conclusion that `src/lexer/mod.rs` needs no changes applies only to the
newline-close form. It does not apply to `echo $(cat << EOF)`, where the scanner emits
the complete inner command as one opaque `CommandSubst` token. In that form,
`tokenize_with_heredocs` sees no top-level `HereDoc` token and therefore does not
consume the following body lines. The resulting `foo`, `bar`, and `EOF` lines are
tokenized as ordinary commands.

The remaining fix must coordinate `src/lexer/mod.rs`, `src/lexer/heredoc.rs`, and the
command-substitution scanner/parser. The design must preserve the opaque substitution
token while carrying nested heredoc metadata/body association, FIFO ordering for
multiple heredocs, quoted delimiters, `<<-`, and the distinction from `<<<`. A
continuation-only patch was tested and rejected because it made the balance predicate
pass without collecting the nested body.

---

## 6. Verification Plan

### Focused Rust test

Add a test in `src/lexer/tests.rs` (or the continuation tests module):

```rust
#[test]
fn heredoc_body_paren_not_closes_command_substitution() {
    // The ) inside the heredoc body must NOT balance the $()
    let input = "echo $(cat <<eof\nhere doc with )\neof\n)";
    let tokens = crate::lexer::tokenize(input);
    // Should contain a HereDocBody with "here doc with )" content
    // NOT an empty HereDocBody followed by orphan )
    let has_here_doc_body = tokens.iter().any(|t| {
        t.kind == crate::lexer::TokenKind::HereDocBody && t.value.contains("here doc with )")
    });
    assert!(has_here_doc_body, "heredoc body should contain the ) character");
}
```

### Official check

After applying the fix:

```bash
MSYS_NO_PATHCONV=1 wsl bash tests/gnu-compat/run-83.sh check comsub-posix
MSYS_NO_PATHCONV=1 wsl bash tests/gnu-compat/run-83.sh check heredoc
```

The comsub-posix check should improve from DIFF (rubash=51 right=23) toward PASS. The heredoc check should improve from DIFF (rubash=163 right=31) — the heredoc3.sub and heredoc7.sub sub-cases in particular should align with GNU behavior.