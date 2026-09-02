use super::*;

impl Executor {
    pub(in crate::executor) fn expand_parameter_word(&self, word: &str) -> String {
        // TODO(subst.c/parse.y): The `word` half of ${parameter:-word},
        // ${parameter:=word}, and ${parameter+word} has quote-aware expansion
        // flags. This covers tilde2.tests while the lexer still discards most
        // quote state.
        let expanded = unescape_remaining_shell_escapes(&decode_parameter_word_quotes(
            &self.expand_embedded_parameters(word),
        ));
        tilde_expand::expand_assignment_tilde_value(&expanded, &self.home_value(), false)
    }

    pub(in crate::executor) fn expand_parameter_word_mut(&mut self, word: &str) -> String {
        let expanded = unescape_remaining_shell_escapes(&decode_parameter_word_quotes(
            &self.expand_embedded_parameters_mut(word),
        ));
        tilde_expand::expand_assignment_tilde_value(&expanded, &self.home_value(), false)
    }

    pub(in crate::executor) fn expand_quoted_parameter_word(&self, word: &str) -> String {
        // TODO(subst.c/parse.y): Quoted parameter expansion should carry
        // CTLESC/CTLQUOTEMARK state from the parser. This preserves the
        // tilde2.tests distinction that quoted default/alternate words do not
        // perform tilde expansion.
        let Some(name) = word
            .strip_prefix("${")
            .and_then(|word| word.strip_suffix('}'))
        else {
            return self.expand_embedded_parameters(word);
        };
        if !braced_parameter_spans_whole_word(word) {
            return self.expand_embedded_parameters(word);
        }

        if let Some((var_name, default)) = name.split_once(":-") {
            if is_parameter_error_name(var_name) {
                return self
                    .parameter_operator_value(var_name)
                    .filter(|value| !value.is_empty())
                    .map(|value| shell_safe_value(&value))
                    .unwrap_or_else(|| {
                        expand_quoted_parameter_operator_word(
                            &self.expand_embedded_parameters(default),
                        )
                    });
            }
        }

        if let Some((var_name, alternate)) = name.split_once(":+") {
            if is_parameter_error_name(var_name) {
                if self
                    .parameter_operator_value(var_name)
                    .is_some_and(|value| !value.is_empty())
                {
                    return expand_quoted_parameter_operator_word(
                        &self.expand_embedded_parameters(alternate),
                    );
                }
                return String::new();
            }
        }

        if let Some((var_name, error_word)) = name.split_once(":?") {
            if is_parameter_error_name(var_name) {
                if self
                    .parameter_operator_value(var_name)
                    .is_some_and(|value| !value.is_empty())
                {
                    return self
                        .parameter_operator_value(var_name)
                        .map(|value| shell_safe_value(&value))
                        .unwrap_or_default();
                }
                return self.expand_embedded_parameters(error_word);
            }
        }

        if let Some((var_name, error_word)) = name.split_once('?') {
            if is_parameter_error_name(var_name) {
                return self
                    .parameter_operator_value(var_name)
                    .map(|value| shell_safe_value(&value))
                    .unwrap_or_else(|| self.expand_embedded_parameters(error_word));
            }
        }

        if let Some((var_name, word)) = name.split_once(":=") {
            if is_parameter_error_name(var_name) {
                return self
                    .parameter_operator_value(var_name)
                    .filter(|value| !value.is_empty())
                    .map(|value| shell_safe_value(&value))
                    .unwrap_or_else(|| self.expand_embedded_parameters(word));
            }
        }

        if let Some((var_name, word)) = name.split_once('=') {
            if is_parameter_error_name(var_name) {
                return self
                    .parameter_operator_value(var_name)
                    .map(|value| shell_safe_value(&value))
                    .unwrap_or_else(|| self.expand_embedded_parameters(word));
            }
        }

        if let Some((var_name, offset, length)) = self.parse_parameter_substring(name) {
            return self.expand_braced_substring_parameter(var_name, offset, length);
        }

        if name.starts_with('#') {
            if let Some(value) = self.expand_braced_indexed_parameter(name) {
                return value;
            }
        }

        if let Some(value) = self.array_element_parameter_value(name) {
            return shell_safe_value(&value);
        }

        if let Some(array_name) = name
            .strip_suffix("[@]")
            .or_else(|| name.strip_suffix("[*]"))
            .filter(|array_name| is_shell_name(array_name))
        {
            return self
                .parameter_array_storage(array_name)
                .map(|value| self.join_array_parameter_values(&value, name))
                .unwrap_or_default();
        }

        if let Some((var_name, alternate)) = name.split_once('+') {
            if is_parameter_error_name(var_name) {
                if self.parameter_operator_value(var_name).is_some() {
                    return expand_quoted_parameter_operator_word(
                        &self.expand_embedded_parameters(alternate),
                    );
                }
                return String::new();
            }
        }

        if let Some((var_name, default)) = name.split_once('-') {
            if is_parameter_error_name(var_name) {
                return self
                    .parameter_operator_value(var_name)
                    .map(|value| shell_safe_value(&value))
                    .unwrap_or_else(|| {
                        expand_quoted_parameter_operator_word(
                            &self.expand_embedded_parameters(default),
                        )
                    });
            }
        }

        if let Some(value) = self.expand_braced_pattern_or_transform_parameter(name) {
            return value;
        }

        if let Some(value) = self.expand_braced_special_or_indirect_parameter(name) {
            return value;
        }

        self.expand_word(word)
    }

    // GNU param_expand: the word of ${var-word}/${var:=word}/${var+word}/...
    // undergoes tilde expansion only in an unquoted expansion context and only
    // when the word itself starts bare (an explicitly quoted `~` stays
    // literal). Tilde applies at the word start, not after colons.
    fn tilde_expand_operator_word(&self, word: &str, context: SubstitutionQuoteContext) -> String {
        if !matches!(context, SubstitutionQuoteContext::Unquoted) {
            return word.to_string();
        }
        if word.starts_with('"') || word.starts_with('\'') || word.starts_with('\\') {
            return word.to_string();
        }
        tilde_expand::expand_assignment_tilde_value(word, &self.home_value(), false)
    }

    pub(in crate::executor) fn expand_quoted_parameter_word_mut(
        &mut self,
        word: &str,
        context: SubstitutionQuoteContext,
    ) -> String {
        // In POSIX mode, a double-quoted `${...}` may close at a `}` inside
        // the apparent word when a single quote is literal (Interp 221).
        // Expand that braced head separately, then continue with the suffix.
        if matches!(context, SubstitutionQuoteContext::DoubleQuoted)
            && self.posix_mode_enabled()
        {
            if let Some(rest) = word.strip_prefix("${") {
                if let Some(close) = matching_parameter_brace_in_context(rest, true, true) {
                    if close + 1 < rest.len() {
                        let braced_end = 2 + close + 1;
                        let head = self.expand_quoted_parameter_word_mut(&word[..braced_end], context);
                        let tail = self.expand_embedded_parameters_mut_with_context(
                            &word[braced_end..],
                            context,
                        );
                        return format!("{head}{tail}");
                    }
                }
            }
        }

        let Some(name) = word
            .strip_prefix("${")
            .and_then(|word| word.strip_suffix('}'))
        else {
            return self.expand_embedded_parameters_mut_with_context(word, context);
        };
        if !braced_parameter_spans_whole_word(word) {
            return self.expand_embedded_parameters_mut_with_context(word, context);
        }

        if let Some((var_name, default)) = name.split_once(":-") {
            if is_parameter_error_name(var_name) {
                return self
                    .parameter_operator_value(var_name)
                    .filter(|value| !value.is_empty())
                    .map(|value| shell_safe_value(&value))
                    .unwrap_or_else(|| {
                        let default = self.tilde_expand_operator_word(default, context);
                        expand_quoted_parameter_operator_word(
                            &self.expand_embedded_parameters_mut_with_context(&default, context),
                        )
                    });
            }
        }

        if let Some((var_name, alternate)) = name.split_once(":+") {
            if is_parameter_error_name(var_name) {
                if self
                    .parameter_operator_value(var_name)
                    .is_some_and(|value| !value.is_empty())
                {
                    let alternate = self.tilde_expand_operator_word(alternate, context);
                    return expand_quoted_parameter_operator_word(
                        &self.expand_embedded_parameters_mut_with_context(&alternate, context),
                    );
                }
                return String::new();
            }
        }

        if let Some((var_name, error_word)) = name.split_once(":?") {
            if is_parameter_error_name(var_name) {
                if self
                    .parameter_operator_value(var_name)
                    .is_some_and(|value| !value.is_empty())
                {
                    return self
                        .parameter_operator_value(var_name)
                        .map(|value| shell_safe_value(&value))
                        .unwrap_or_default();
                }
                let error_word = self.tilde_expand_operator_word(error_word, context);
                return self.expand_embedded_parameters_mut_with_context(&error_word, context);
            }
        }

        if let Some((var_name, error_word)) = name.split_once('?') {
            if is_parameter_error_name(var_name) {
                return self
                    .parameter_operator_value(var_name)
                    .map(|value| shell_safe_value(&value))
                    .unwrap_or_else(|| {
                        let error_word = self.tilde_expand_operator_word(error_word, context);
                        self.expand_embedded_parameters_mut_with_context(&error_word, context)
                    });
            }
        }

        if let Some((var_name, word)) = name.split_once(":=") {
            if is_parameter_error_name(var_name) {
                return self
                    .parameter_operator_value(var_name)
                    .filter(|value| !value.is_empty())
                    .map(|value| shell_safe_value(&value))
                    .unwrap_or_else(|| {
                        let word = self.tilde_expand_operator_word(word, context);
                        self.expand_embedded_parameters_mut_with_context(&word, context)
                    });
            }
        }

        if let Some((var_name, word)) = name.split_once('=') {
            if is_parameter_error_name(var_name) {
                return self
                    .parameter_operator_value(var_name)
                    .map(|value| shell_safe_value(&value))
                    .unwrap_or_else(|| {
                        let word = self.tilde_expand_operator_word(word, context);
                        self.expand_embedded_parameters_mut_with_context(&word, context)
                    });
            }
        }

        if let Some((var_name, offset, length)) = self.parse_parameter_substring_mut(name) {
            return self.expand_braced_substring_parameter(var_name, offset, length);
        }

        if name.starts_with('#') {
            if let Some(value) = self.expand_braced_indexed_parameter(name) {
                return value;
            }
        }

        if let Some(value) = self.array_element_parameter_value(name) {
            return shell_safe_value(&value);
        }

        if let Some(array_name) = name
            .strip_suffix("[@]")
            .or_else(|| name.strip_suffix("[*]"))
            .filter(|array_name| is_shell_name(array_name))
        {
            return self
                .parameter_array_storage(array_name)
                .map(|value| self.join_array_parameter_values(&value, name))
                .unwrap_or_default();
        }

        if let Some((var_name, alternate)) = name.split_once('+') {
            if is_parameter_error_name(var_name) {
                if self.parameter_operator_value(var_name).is_some() {
                    let alternate = self.tilde_expand_operator_word(alternate, context);
                    let expanded =
                        self.expand_embedded_parameters_mut_with_context(&alternate, context);
                    return expand_quoted_parameter_operator_word(&expanded);
                }
                return String::new();
            }
        }

        if let Some((var_name, default)) = name.split_once('-') {
            if is_parameter_error_name(var_name) {
                return self
                    .parameter_operator_value(var_name)
                    .map(|value| shell_safe_value(&value))
                    .unwrap_or_else(|| {
                        let default = self.tilde_expand_operator_word(default, context);
                        expand_quoted_parameter_operator_word(
                            &self.expand_embedded_parameters_mut_with_context(&default, context),
                        )
                    });
            }
        }

        if let Some(value) = self.expand_braced_pattern_or_transform_parameter(name) {
            return value;
        }

        if let Some(value) = self.expand_braced_special_or_indirect_parameter(name) {
            return value;
        }

        self.expand_word(word)
    }

    fn expand_parameter_assignment_value_mut(&mut self, value: &str) -> String {
        const PROTECTED_ESCAPED_SPACE: char = '\x13';
        let protected = value.replace("\\ ", &PROTECTED_ESCAPED_SPACE.to_string());
        self.expand_parameter_word_mut(&protected)
            .replace(PROTECTED_ESCAPED_SPACE, "\\ ")
    }

    pub(in crate::executor) fn apply_parameter_assignment_expansions_in_word(
        &mut self,
        word: &str,
    ) {
        let mut rest = word;
        while let Some(start) = rest.find("${") {
            rest = &rest[start + 2..];
            let Some(end) = matching_parameter_brace(rest) else {
                break;
            };
            let inner = &rest[..end];
            self.apply_parameter_assignment_expansion(inner);
            rest = &rest[end + 1..];
        }
    }

    pub(in crate::executor) fn apply_parameter_assignment_expansion(&mut self, inner: &str) {
        if let Some((name, value)) = inner.split_once(":=") {
            if self
                .parameter_operator_value(name)
                .is_some_and(|value| !value.is_empty())
            {
                return;
            }
            let value = self.expand_parameter_word_mut(value);
            if self.apply_array_element_parameter_assignment(name, value.clone()) {
                return;
            }
            if self.apply_indirect_parameter_assignment(name, value.clone()) {
                return;
            }
            if !is_shell_name(name) {
                return;
            }
            self.apply_shell_assignment(name, value);
            return;
        }

        if let Some((name, value)) = inner.split_once('=') {
            if self.parameter_operator_value(name).is_some() {
                return;
            }
            let value = self.expand_parameter_assignment_value_mut(value);
            if self.apply_array_element_parameter_assignment(name, value.clone()) {
                return;
            }
            if self.apply_indirect_parameter_assignment(name, value.clone()) {
                return;
            }
            if !is_shell_name(name) {
                return;
            }
            self.apply_shell_assignment(name, value);
        }
    }

    fn apply_indirect_parameter_assignment(&mut self, name: &str, value: String) -> bool {
        let Some(indirect_name) = name.strip_prefix('!') else {
            return false;
        };
        if self.nameref_target_name(indirect_name).is_some() {
            return false;
        }
        let Some(target_name) = self.env_vars.get(indirect_name).cloned() else {
            return false;
        };
        if self.apply_array_element_parameter_assignment(&target_name, value.clone()) {
            return true;
        }
        if !is_shell_name(&target_name) {
            return false;
        }
        self.apply_shell_assignment(&target_name, value);
        true
    }
}

fn decode_double_quotes_in_quoted_parameter_word(word: &str) -> String {
    let mut output = String::new();
    let chars = word.chars().collect::<Vec<_>>();
    let mut index = 0usize;
    while index < chars.len() {
        if chars[index] != '"' {
            output.push(chars[index]);
            index += 1;
            continue;
        }

        index += 1;
        while index < chars.len() {
            match chars[index] {
                '"' => {
                    index += 1;
                    break;
                }
                '\\' if matches!(chars.get(index + 1), Some('\\' | '"' | '$' | '`' | '\n')) => {
                    index += 1;
                    if index < chars.len() && chars[index] != '\n' {
                        output.push(chars[index]);
                    }
                    index += 1;
                }
                ch => {
                    output.push(ch);
                    index += 1;
                }
            }
        }
    }
    output
}

fn expand_quoted_parameter_operator_word(word: &str) -> String {
    unescape_remaining_shell_escapes(&decode_double_quotes_in_quoted_parameter_word(word))
}
