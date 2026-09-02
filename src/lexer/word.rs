use super::classification::{
    assignment_value_is_quoted, has_unquoted_assignment_equal, is_assignment, is_keyword,
    mark_quoted_assignment_value, quoted_literal_tilde,
};
use super::quotes::{remove_shell_quotes, remove_shell_quotes_outside_backticks};
use super::scanner::Lexer;
use super::token::{Token, TokenKind};

impl<'a> Lexer<'a> {
    pub(super) fn finish_word_token(&mut self, start: usize, allow_keyword: bool) -> Token {
        if self.word_so_far_ends_extglob_operator(start) && self.peek() == Some('(') {
            self.skip_extglob_group();
        }
        self.skip_word();
        let raw = self.slice(start);
        // Only real assignment words (`a=$(cmd)`) preserve quotes verbatim so
        // the RHS quote state survives to assignment expansion. Ordinary words
        // that merely contain `=` and `$(` (e.g. `echo "B: $(printf 'v=[%s]'
        // "$(printf 'mid')")"`) must still go through quote removal, otherwise
        // the trailing `"` leaks into the expanded argument.
        let value = if is_assignment(&raw) && raw.contains("$(") {
            // TODO(parse.y/subst.c): Preserve quotes inside `$()` while
            // assignment-word quote removal is still token-local.
            raw.to_string()
        } else if raw.starts_with("$((") {
            // GNU keeps the text of a `$((...))` expansion verbatim at the
            // word level; the arithmetic stage applies its own double-quote
            // rules there (`\"` is a literal quote, `(( "1" ))` loses the
            // quotes in the evaluator, not by word-level quote removal).
            raw.to_string()
        } else if is_assignment(&raw) && raw.contains('`') {
            // TODO(parse.y/subst.c): Assignment-word quote removal must not
            // consume quotes inside command substitutions. Preserve the
            // backquote body for the substitution stage.
            remove_shell_quotes_outside_backticks(raw)
        } else {
            remove_shell_quotes(raw)
        };
        let kind = if allow_keyword && is_keyword(raw) {
            TokenKind::Keyword
        } else if is_assignment(&value) && has_unquoted_assignment_equal(raw) {
            TokenKind::Assignment
        } else {
            TokenKind::Word
        };
        let value = if quoted_literal_tilde(raw, &value) {
            // TODO(parse.y/subst.c): Preserve quote state as WORD_DESC flags.
            // This prevents quoted literal `~` from undergoing tilde
            // expansion before builtins like `printf %q` see it.
            format!("\x1b{value}")
        } else if kind == TokenKind::Assignment && assignment_value_is_quoted(raw) {
            // TODO(parse.y/subst.c): Replace this narrow quoted-RHS marker
            // with WORD_DESC quote flags. It lets assignment tilde expansion
            // distinguish `a=~/x` from `a="~/x"` without leaking syntax to
            // builtins.
            mark_quoted_assignment_value(raw, &value)
        } else if kind == TokenKind::Word
            && is_assignment(&value)
            && assignment_value_is_quoted(raw)
        {
            // A fully quoted assignment-looking argument, such as
            // `"SHELL=~/bash"`, remains a normal word but its RHS quote state
            // still suppresses the assignment-word tilde pass.
            mark_quoted_assignment_value(raw, &value)
        } else if raw.starts_with('"') && raw.ends_with('"') && raw.contains("${") {
            // TODO(parse.y/subst.c): Preserve full quote state on WORD_DESC
            // instead of a sentinel. This narrow marker lets expansion
            // distinguish "${v:-~}" from ${v:-~} for upstream tilde2.tests.
            format!("\x1d{value}")
        } else {
            value
        };
        let raw = if kind == TokenKind::Assignment && self.peek() == Some('(') {
            // `name=(...)` is a compound array assignment word in Bash: the
            // opening paren must be adjacent to the `=` with no space.
            // Mark the raw so the parser only treats an adjacent `(` as the
            // array-assignment form (`a= (1 2)` is a syntax error in Bash).
            format!("{raw}(")
        } else {
            raw.to_string()
        };
        Token::new_with_raw(kind, &value, &raw, start)
    }

    pub(super) fn skip_word(&mut self) {
        let mut extglob_operator = false;
        let array_assignment = self.looks_like_array_element_assignment();
        let mut array_subscript_depth = 0usize;
        let mut array_value_paren_depth = 0usize;
        while let Some(c) = self.peek() {
            let in_array_value = array_assignment && array_value_paren_depth > 0;
            if " \t\n|&;<>(){}".contains(c)
                && c != '}'
                && !(array_assignment && array_subscript_depth > 0 && c.is_ascii_whitespace())
                && !(in_array_value && c.is_ascii_whitespace())
                && !(in_array_value && matches!(c, '(' | ')'))
                && !(array_assignment
                    && array_subscript_depth == 0
                    && array_value_paren_depth == 0
                    && c == '('
                    && self.input[..self.position].ends_with('='))
            {
                if c == '(' && extglob_operator {
                    self.skip_extglob_group();
                    extglob_operator = false;
                    continue;
                }
                if c == '{' {
                    self.advance();
                    self.skip_brace();
                    extglob_operator = false;
                    continue;
                }
                break;
            }
            match c {
                '(' if array_assignment
                    && array_subscript_depth == 0
                    && array_value_paren_depth == 0
                    && self.input[..self.position].ends_with('=') =>
                {
                    self.advance();
                    array_value_paren_depth = 1;
                }
                '(' if in_array_value => {
                    self.advance();
                    array_value_paren_depth += 1;
                }
                ')' if in_array_value => {
                    self.advance();
                    array_value_paren_depth = array_value_paren_depth.saturating_sub(1);
                }
                '`' => {
                    // TODO(parse.y/subst.c): Command substitution is part of
                    // the surrounding word. Keeping it atomic is required for
                    // assignment words such as v=`echo x`.
                    self.advance();
                    self.skip_backtick();
                    extglob_operator = false;
                }
                '\'' => {
                    self.advance();
                    self.skip_single();
                    extglob_operator = false;
                }
                '"' => {
                    self.advance();
                    self.skip_double();
                    extglob_operator = false;
                }
                '\\' => {
                    self.advance();
                    self.advance();
                    extglob_operator = false;
                }
                '[' if array_assignment => {
                    self.advance();
                    array_subscript_depth += 1;
                    extglob_operator = false;
                }
                ']' if array_assignment && array_subscript_depth > 0 => {
                    self.advance();
                    array_subscript_depth -= 1;
                    extglob_operator = false;
                }
                '$' => {
                    self.advance();
                    match self.peek() {
                        Some('{') => {
                            self.advance();
                            self.skip_braced(false);
                        }
                        Some('(') => {
                            self.advance();
                            if self.peek() == Some('(') {
                                self.advance();
                                self.skip_arith_paren();
                            } else {
                                self.skip_cmd_subst();
                            }
                        }
                        Some('[') => {
                            self.advance();
                            self.skip_arith_bracket();
                        }
                        Some('\'') => {
                            self.advance();
                            self.skip_ansi_c_single();
                        }
                        _ => {}
                    }
                    extglob_operator = false;
                }
                _ => {
                    self.advance();
                    extglob_operator = matches!(c, '@' | '*' | '+' | '?' | '!');
                }
            }
        }
    }

    fn looks_like_array_element_assignment(&self) -> bool {
        let rest = &self.input[self.position..];
        if self.position == 0
            || self.input.as_bytes()[self.position - 1] != b'[' && !rest.starts_with('[')
        {
            return false;
        }
        let open = if rest.starts_with('[') {
            self.position
        } else {
            return false;
        };
        let name_end = open;
        let mut name_start = name_end;
        while name_start > 0 {
            let ch = self.input.as_bytes()[name_start - 1] as char;
            if ch == '_' || ch.is_ascii_alphanumeric() {
                name_start -= 1;
            } else {
                break;
            }
        }
        let name = &self.input[name_start..name_end];
        let mut name_chars = name.chars();
        let Some(first) = name_chars.next() else {
            return false;
        };
        if !(first == '_' || first.is_ascii_alphabetic()) {
            return false;
        }
        if !name_chars.all(|ch| ch == '_' || ch.is_ascii_alphanumeric()) {
            return false;
        }
        let mut depth = 1usize;
        let mut escaped = false;
        let mut close = None;
        for (offset, ch) in rest.char_indices().skip(1) {
            if escaped {
                escaped = false;
                continue;
            }
            if ch == '\\' {
                escaped = true;
                continue;
            }
            match ch {
                '[' => depth += 1,
                ']' => {
                    depth = depth.saturating_sub(1);
                    if depth == 0 {
                        close = Some(offset);
                        break;
                    }
                }
                _ => {}
            }
        }
        close
            .and_then(|offset| rest.get(offset + 1..))
            .is_some_and(|tail| tail.starts_with('='))
    }

    fn word_so_far_ends_extglob_operator(&self, start: usize) -> bool {
        self.slice(start)
            .chars()
            .last()
            .is_some_and(|ch| matches!(ch, '@' | '*' | '+' | '?' | '!'))
    }

    fn skip_extglob_group(&mut self) {
        if self.peek() != Some('(') {
            return;
        }

        self.advance();
        let mut depth = 1usize;
        while let Some(c) = self.advance() {
            match c {
                '(' => depth += 1,
                ')' => {
                    depth -= 1;
                    if depth == 0 {
                        break;
                    }
                }
                '`' => self.skip_backtick(),
                '\'' => self.skip_single(),
                '"' => self.skip_double(),
                '\\' => {
                    self.advance();
                }
                _ => {}
            }
        }
    }
}
