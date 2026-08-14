use super::*;

impl Executor {
    pub(in crate::executor) fn reparse_reserved_word_aliases(&self, source: &str) -> Option<String> {
        let mut tokens = crate::lexer::tokenize(source);
        let mut changed = false;
        for token in &mut tokens {
            if !matches!(token.kind, crate::lexer::TokenKind::Word | crate::lexer::TokenKind::Keyword)
            {
                continue;
            }
            let Some(alias) = self.aliases.get(&token.value) else {
                continue;
            };
            let value = alias.value.trim();
            if !matches!(value, "if" | "then" | "elif" | "else" | "fi") {
                continue;
            }
            token.value = value.to_string();
            token.raw = value.to_string();
            changed = true;
        }
        if !changed {
            return None;
        }
        Some(
            tokens
                .iter()
                .filter(|token| token.kind != crate::lexer::TokenKind::Eof)
                .map(|token| token.raw.as_str())
                .collect::<Vec<_>>()
                .join(" "),
        )
    }

    pub(in crate::executor) fn report_arithmetic_error_with_label(
        &self,
        label: &str,
        expression: &str,
    ) {
        if let Some(token) = arithmetic_division_by_zero_token(expression) {
            eprintln!(
                "{}{}: {expression}: division by 0 (error token is \"{token}\")",
                self.diagnostic_prefix(),
                label
            );
        } else if let Some(message) =
            crate::executor::arithmetic::arithmetic_error_message(expression)
        {
            eprintln!("{}{}: {message}", self.diagnostic_prefix(), label);
        }
    }

    pub(in crate::executor) fn report_arithmetic_error(&self, expression: &str) {
        self.report_arithmetic_error_with_label("((", expression);
    }

    pub(in crate::executor) fn report_let_arithmetic_error(&self, expression: &str) {
        self.report_arithmetic_error_with_label("let", expression);
    }

    pub(in crate::executor) fn execute_arithmetic_command(&mut self, cmd: &CommandNode) -> i32 {
        let expression = cmd.words.get(1).map(String::as_str).unwrap_or_default();
        match self.eval_arithmetic_command_value(expression) {
            Some(0) => 1,
            Some(_) => 0,
            None => {
                self.report_arithmetic_error(expression);
                1
            }
        }
    }

    pub(in crate::executor) fn execute_let(&mut self, expressions: &[String]) -> i32 {
        if expressions.is_empty() {
            eprintln!("{}let: expression expected", self.diagnostic_prefix());
            return 1;
        }

        let mut value = None;
        let mut index = 0;
        while index < expressions.len() {
            let mut expression = expressions[index].clone();
            if expression.contains(COMPOUND_ASSIGNMENT_MARKER)
                && expressions
                    .get(index + 1)
                    .is_some_and(|word| arithmetic_assignment_suffix(word))
            {
                expression.push_str(&expressions[index + 1]);
                index += 1;
            }
            let expression = arithmetic_expression_arg(&expression);
            value = self.eval_arithmetic_command_value(&expression);
            if value.is_none() {
                self.report_let_arithmetic_error(&expression);
                return 1;
            }
            index += 1;
        }
        match value {
            Some(0) | None => 1,
            Some(_) => 0,
        }
    }

    pub(crate) fn expand_aliases(&self, words: &[String]) -> Vec<String> {
        if !self.alias_expansion_enabled() {
            return words.to_vec();
        }

        let mut expanded = Vec::new();
        let mut expand_next = true;

        for word in words {
            if expand_next {
                let mut seen = Vec::new();
                let (mut alias_words, alias_expand_next) = self.expand_alias_word(word, &mut seen);
                if alias_words.is_empty() && !self.aliases.contains_key(word) {
                    expanded.push(word.clone());
                } else {
                    expanded.append(&mut alias_words);
                }
                expand_next = alias_expand_next;
            } else {
                expanded.push(word.clone());
                expand_next = false;
            }
        }

        expanded
    }

    /// Alias expansion that honours quote state: Bash never expands an alias
    /// whose word is quoted (`'hi'`, `"hi"` stay literal). `raws` carries the
    /// per-word raw text from word metadata so the caller can distinguish
    /// `hi` from `'hi'` after quote removal.
    pub(in crate::executor) fn expand_aliases_with_raw(
        &self,
        words: &[String],
        raws: &[Option<&str>],
    ) -> Vec<String> {
        if !self.alias_expansion_enabled() {
            return words.to_vec();
        }

        let mut expanded = Vec::new();
        let mut expand_next = true;

        for (index, word) in words.iter().enumerate() {
            let raw = raws.get(index).copied().flatten();
            if expand_next && !crate::executor::command_prepare::raw_word_is_quoted(raw) {
                let mut seen = Vec::new();
                let (mut alias_words, alias_expand_next) = self.expand_alias_word(word, &mut seen);
                if alias_words.is_empty() && !self.aliases.contains_key(word) {
                    expanded.push(word.clone());
                } else {
                    expanded.append(&mut alias_words);
                }
                expand_next = alias_expand_next;
            } else {
                expanded.push(word.clone());
                expand_next = false;
            }
        }

        expanded
    }

    pub(in crate::executor) fn expand_aliases_preserving_reserved(
        &self,
        words: &[String],
    ) -> Vec<String> {
        if !self.alias_expansion_enabled() {
            return words.to_vec();
        }

        // TODO(parse.y/alias.c): In POSIX mode Bash does not alias reserved
        // words. This keeps just enough parser-state awareness for alias7.sub.
        let mut expanded = Vec::new();
        let mut expand_next = true;

        for word in words {
            if expand_next && !is_reserved_word(word) {
                let mut seen = Vec::new();
                let (mut alias_words, alias_expand_next) = self.expand_alias_word(word, &mut seen);
                expanded.append(&mut alias_words);
                expand_next = alias_expand_next;
            } else {
                expanded.push(word.clone());
                expand_next = false;
            }
        }

        expanded
    }

    pub(in crate::executor) fn execute_parser_level_alias(
        &mut self,
        cmd: &CommandNode,
    ) -> Result<bool, ExecuteError> {
        if !self.alias_expansion_enabled() {
            return Ok(false);
        }

        // TODO(parse.y/alias.c): GNU Bash pushes alias text back into the
        // parser input stream (`alias_expand_token` + `push_string`). This
        // reparses complex alias values at command position so aliases that
        // introduce `;`, newlines, or redirections behave closer to Bash until
        // Rubash has a real parser input stack.
        let Some(word) = cmd.words.first() else {
            return Ok(false);
        };

        if self.expanding_aliases.iter().any(|alias| alias == word) {
            return Ok(false);
        }

        // Bash never expands a quoted word as an alias (`'hi'` / `"hi"`).
        let word_is_quoted = cmd
            .word_metadata
            .first()
            .map(|metadata| {
                crate::executor::command_prepare::raw_word_is_quoted(Some(&metadata.raw))
            })
            .unwrap_or(false);
        if word_is_quoted {
            return Ok(false);
        }

        let Some(alias) = self.aliases.get(word).cloned() else {
            return Ok(false);
        };

        if !needs_parser_level_alias_expansion(&alias.value) {
            return Ok(false);
        }

        let mut source = alias.value.replace('\x1f', "$");
        if !cmd.words[1..].is_empty()
            && (has_unclosed_quote(&alias.value)
                || (!source.ends_with(' ') && !source.ends_with('\t')))
        {
            source.push(' ');
        }
        source.push_str(&cmd.words[1..].join(" "));

        self.expanding_aliases.push(word.clone());
        let tokens = crate::lexer::tokenize(&source);
        let ast = crate::parser::parse(&tokens);
        let result = self.execute_ast(&ast);
        self.expanding_aliases.pop();
        result.map(|_| true)
    }

    pub(in crate::executor) fn alias_parser_source(
        &self,
        word: &str,
        rest: &[String],
    ) -> Option<String> {
        let mut seen = Vec::new();
        let mut source = self.alias_parser_source_inner(word, rest, &mut seen)?;
        while let Some((first, remainder)) = split_first_shell_word(&source) {
            let remainder = remainder.to_string();
            if seen.iter().any(|seen_word| seen_word == &first) {
                break;
            }
            let Some(expanded) = self.alias_parser_source_inner(&first, &[], &mut seen) else {
                break;
            };
            source = expanded;
            if !remainder.is_empty() {
                if !source.ends_with(' ') && !source.ends_with('\t') && !source.ends_with('\n') {
                    source.push('\n');
                }
                source.push_str(&remainder);
            }
        }
        Some(source)
    }

    pub(in crate::executor) fn alias_parser_source_inner(
        &self,
        word: &str,
        rest: &[String],
        seen: &mut Vec<String>,
    ) -> Option<String> {
        if seen.iter().any(|seen_word| seen_word == word) {
            return None;
        }
        let alias = self.aliases.get(word)?;
        if !needs_parser_level_alias_expansion(&alias.value)
            && !matches!(alias.value.trim(), "if" | "then" | "elif" | "else" | "fi")
        {
            return None;
        }

        seen.push(word.to_string());
        let mut source = alias.value.replace('\x1f', "$");
        if !rest.is_empty()
            && (has_unclosed_quote(&alias.value)
                || (!source.ends_with(' ') && !source.ends_with('\t')))
        {
            source.push(' ');
        }
        source.push_str(&rest.join(" "));
        Some(source)
    }

    pub(in crate::executor) fn expand_alias_word(
        &self,
        word: &str,
        seen: &mut Vec<String>,
    ) -> (Vec<String>, bool) {
        // TODO(alias.c/alias.h/parse.y): Bash marks AL_BEINGEXPANDED in
        // parse.y::alias_expand_token and re-reads parser input. This executor-level
        // approximation preserves AL_EXPANDNEXT and recursion suppression, but it
        // cannot make redirections or compound commands introduced by aliases parse
        // exactly like GNU Bash yet.
        if seen.iter().any(|seen_word| seen_word == word) {
            return (vec![word.to_string()], false);
        }

        let Some(alias) = self.aliases.get(word) else {
            return (vec![word.to_string()], false);
        };

        if alias.value.is_empty() {
            return (Vec::new(), false);
        }

        seen.push(word.to_string());
        let mut parts: Vec<String> = alias.value.split_whitespace().map(str::to_string).collect();

        if let Some(first) = parts.first().cloned() {
            let (mut first_expanded, nested_expand_next) = self.expand_alias_word(&first, seen);
            parts.remove(0);
            first_expanded.extend(parts);
            // TODO(alias.c/parse.y): Bash preserves AL_EXPANDNEXT through
            // chained alias expansion. This approximates that propagation for
            // nested aliases like `a2=a1`, `a1='echo '`.
            (first_expanded, alias.expand_next || nested_expand_next)
        } else {
            (Vec::new(), alias.expand_next)
        }
    }
}
