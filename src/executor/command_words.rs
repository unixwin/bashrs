use super::*;

impl Executor {
    pub(in crate::executor) fn update_underscore_parameter(&mut self, cmd: &CommandNode) {
        if let Some(value) = cmd.words.last() {
            self.env_vars.insert("_".to_string(), value.clone());
        }
    }

    pub(in crate::executor) fn removes_unquoted_null_word(
        &self,
        cmd: &CommandNode,
        index: usize,
    ) -> bool {
        if cmd.words.first().is_some_and(|word| word == "[[") {
            return false;
        }

        cmd.word_kinds
            .get(index)
            .is_some_and(|kind| *kind == TokenKind::Variable)
    }

    pub(in crate::executor) fn splits_unquoted_expanded_word(
        &self,
        cmd: &CommandNode,
        index: usize,
        expanded: &str,
    ) -> bool {
        // A word wrapped in quotes (e.g. `"$(cmd) extra"`) keeps its spaces
        // together: quote removal happens after field splitting in Bash, so
        // quoted words must not be split even when they expand to whitespace.
        let word_is_quoted = cmd
            .word_metadata
            .get(index)
            .map(|metadata| {
                crate::executor::command_prepare::raw_word_is_quoted(Some(&metadata.raw))
            })
            .unwrap_or(false);
        let unquoted_variable = cmd
            .word_kinds
            .get(index)
            .is_some_and(|kind| *kind == TokenKind::Variable)
            && !word_is_quoted;
        let unquoted_dynamic_parameter = unquoted_variable
            && cmd
                .words
                .get(index)
                .and_then(|word| dynamic_scalar_parameter_name(word))
                .is_some_and(|name| self.dynamic_parameter_is_set(name));
        let unquoted_command_substitution = cmd
            .word_metadata
            .get(index)
            .map(|metadata| metadata.raw.as_str())
            .or_else(|| cmd.words.get(index).map(String::as_str))
            .is_some_and(word_has_unquoted_command_substitution);
        let unquoted_indirect_name_list = cmd
            .words
            .get(index)
            .is_some_and(|word| word_is_unquoted_indirect_name_list(word));
        let unquoted_embedded_parameter = !word_is_quoted
            && cmd
                .word_metadata
                .get(index)
                .map(|metadata| metadata.raw.as_str())
                .or_else(|| cmd.words.get(index).map(String::as_str))
                .is_some_and(raw_word_has_unquoted_parameter_expansion);

        let field_split_would_split = self.field_split_values(expanded).len() > 1;

        ((unquoted_variable && !unquoted_dynamic_parameter && field_split_would_split)
            || (unquoted_embedded_parameter && field_split_would_split)
            || (unquoted_command_substitution && expanded.contains(char::is_whitespace))
            || (unquoted_indirect_name_list && expanded.contains(char::is_whitespace)))
            && (field_split_would_split || expanded.split_whitespace().nth(1).is_some())
    }

    pub(in crate::executor) fn expand_for_word_values_result(
        &mut self,
        word: &str,
        raw: Option<&str>,
        metadata: Option<&WordMetadata>,
    ) -> Result<Vec<String>, String> {
        let suppress_glob = word.starts_with('\x1b')
            || word.starts_with('\x1d')
            || super::command_prepare::raw_word_suppresses_pathname_expansion(raw, metadata);
        if let Some(values) = self.quoted_positional_at_word_values_with_raw(word, raw, None) {
            return Ok(values);
        }
        if let Some(values) = self.array_at_word_values(word) {
            if word_is_unquoted_array_list_expansion(word) {
                return Ok(field_split_array_values_with_ifs(
                    values,
                    self.env_vars.get("IFS").map(String::as_str),
                ));
            }
            return Ok(values);
        }
        if self.is_brace_expand_enabled() && !word.contains("${") {
            let braced = super::command_prepare::expand_braces_with_optional_raw(word, raw);
            if braced.len() > 1 {
                let values = braced
                    .into_iter()
                    .map(|word| self.expand_for_brace_word_values(&word, raw, suppress_glob))
                    .collect::<Result<Vec<_>, _>>()?
                    .into_iter()
                    .flatten()
                    .collect();
                return Ok(values);
            }
        }

        self.expand_for_brace_word_values(word, raw, suppress_glob)
    }

    fn expand_for_brace_word_values(
        &mut self,
        word: &str,
        raw: Option<&str>,
        suppress_glob: bool,
    ) -> Result<Vec<String>, String> {
        let mut expanded = self.expand_word(word);
        if expanded.contains("<(") || expanded.contains(">(") {
            expanded = self
                .materialize_assignment_process_substitutions(&expanded)
                .unwrap_or(expanded);
        }
        if for_word_has_unquoted_expansion(word, raw) {
            return Ok(expanded.split_whitespace().map(str::to_string).collect());
        }
        if suppress_glob {
            return Ok(vec![expanded]);
        }
        // Apply glob expansion for for-loop words
        match glob::pathname_expand_word(&expanded, &self.env_vars) {
            glob::PathnameExpansion::Matches(matches) => Ok(matches),
            glob::PathnameExpansion::NoMatch => Ok(vec![expanded]),
            glob::PathnameExpansion::Fail(pattern) => Err(pattern),
        }
    }

    pub(in crate::executor) fn field_split_values(&self, value: &str) -> Vec<String> {
        field_split_values_with_ifs(value, self.env_vars.get("IFS").map(String::as_str))
    }

    pub(in crate::executor) fn expand_escaped_indirect_parameter_literal(
        &self,
        value: &str,
    ) -> Option<String> {
        let marker = "\\${$";
        let start = value.find(marker)?;
        let mut output = String::new();
        output.push_str(&value[..start]);
        let mut index = start + marker.len();
        let rest = &value[index..];
        let mut name = String::new();
        for ch in rest.chars() {
            if !is_shell_name_char(ch) {
                break;
            }
            name.push(ch);
            index += ch.len_utf8();
        }
        if name.is_empty() {
            return None;
        }
        let tail = &value[index..];
        let end = tail.find('}')?;
        let resolved = self.expand_embedded_parameters(&format!("${name}"));
        output.push_str("${");
        output.push_str(&resolved);
        output.push_str(&tail[..end]);
        output.push('}');
        output.push_str(&tail[end + 1..]);
        Some(output)
    }
}

fn dynamic_scalar_parameter_name(word: &str) -> Option<&str> {
    let name = word
        .strip_prefix("${")
        .and_then(|word| word.strip_suffix('}'))
        .or_else(|| word.strip_prefix('$'))?;
    is_shell_name(name).then_some(name)
}

fn word_is_unquoted_indirect_name_list(word: &str) -> bool {
    let Some(inner) = word
        .strip_prefix("${!")
        .and_then(|word| word.strip_suffix('}'))
    else {
        return false;
    };

    inner
        .strip_suffix("[@]")
        .or_else(|| inner.strip_suffix("[*]"))
        .is_some_and(|name| !name.is_empty())
        || inner
            .strip_suffix('*')
            .or_else(|| inner.strip_suffix('@'))
            .is_some_and(|prefix| !prefix.is_empty())
}

fn raw_word_has_unquoted_parameter_expansion(raw: &str) -> bool {
    let chars = raw.chars().collect::<Vec<_>>();
    let mut index = 0usize;
    while index < chars.len() {
        if chars[index] == '\\' {
            index += 2;
            continue;
        }
        if chars[index] == '$' {
            match chars.get(index + 1).copied() {
                Some('{') => {
                    if chars.get(index + 2).is_some_and(|ch| {
                        is_shell_name_start(*ch)
                            || matches!(*ch, '@' | '*' | '#' | '?' | '$' | '!' | '-' | '0')
                    }) {
                        return true;
                    }
                }
                Some(ch)
                    if is_shell_name_start(ch)
                        || matches!(ch, '@' | '*' | '#' | '?' | '$' | '!' | '-' | '0') =>
                {
                    return true;
                }
                _ => {}
            }
        }
        index += 1;
    }
    false
}
