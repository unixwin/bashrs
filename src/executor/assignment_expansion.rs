use super::*;

#[derive(Debug, Eq, PartialEq)]
pub(in crate::executor) struct AssignmentExpansionResult {
    pub(in crate::executor) value: String,
    pub(in crate::executor) substitution_status: Option<i32>,
    pub(in crate::executor) arithmetic_error: bool,
    pub(in crate::executor) arithmetic_nonfatal_error: bool,
}

impl Executor {
    pub(in crate::executor) fn expand_assignment_value_result(
        &mut self,
        value: &str,
    ) -> AssignmentExpansionResult {
        self.last_command_substitution_status.set(None);
        let expanded = self.expand_assignment_value(value);
        let substitution_status = self.last_command_substitution_status.get();
        self.last_command_substitution_status.set(None);
        let arithmetic_error = self.arithmetic_expansion_error.replace(false);
        let arithmetic_nonfatal_error = self.arithmetic_nonfatal_error.replace(false);
        AssignmentExpansionResult {
            value: expanded,
            substitution_status,
            arithmetic_error,
            arithmetic_nonfatal_error,
        }
    }

    /// Raw double quotes surviving in a token value are single-quote DATA at
    /// this point: remove_shell_quotes already consumed the active ones, so a
    /// remaining `"` can only come from a single-quoted segment (GNU keeps it
    /// literal in the assigned value). The downstream embedded-parameter
    /// re-scan would re-process it as syntax, so carry those quotes with the
    /// internal DATA_DOUBLE_QUOTE marker across expansion and restore them on
    /// the way out (assignment_expansion hoist/restore contract).
    pub(in crate::executor) fn expand_assignment_value(&mut self, value: &str) -> String {
        // Only hoist when no command-substitution payload is present: quotes
        // inside a $()/backtick body are syntax for the nested parse, not data.
        if !value.contains('"')
            || value.contains('`')
            || value.contains("$(")
            || contains_command_substitution_payload(value)
        {
            return self.expand_assignment_value_inner(value);
        }
        const DQ_DATA: &str = "\u{E001}";
        let expanded = self.expand_assignment_value_inner(&value.replace('"', DQ_DATA));
        expanded.replace(DQ_DATA, "\"")
    }

    fn expand_assignment_value_inner(&mut self, value: &str) -> String {
        if !value.contains("$(") && !value.contains('`') {
            if let Some(array_value) = normalize_single_element_array_assignment(value) {
                return array_value;
            }
        }

        let quoted = value.starts_with(tilde_expand::QUOTED_ASSIGNMENT_VALUE);
        let value = tilde_expand::strip_assignment_quote_marker(value);
        let compound_assignment = value.starts_with(COMPOUND_ASSIGNMENT_MARKER);
        let value = value
            .strip_prefix(COMPOUND_ASSIGNMENT_MARKER)
            .unwrap_or(value);
        if value.contains("\\$(") {
            let literal = if quoted {
                strip_matching_quotes(value)
            } else {
                value
            };
            return unescape_remaining_shell_escapes(literal);
        }
        if quoted && value.contains(":$((") {
            return self.expand_quoted_prompt_arithmetic_assignment(value);
        }
        let value = if quoted && (value.contains("$(") || value.contains('`')) {
            strip_matching_quotes(value)
        } else {
            value
        };
        if quoted {
            if let Some(expanded) = self.expand_quoted_array_assignment_value(value) {
                return expanded;
            }
        }
        if compound_assignment
            && value.starts_with('(')
            && value.ends_with(')')
            && !value.contains('$')
            && !value.contains('`')
        {
            return format!("{COMPOUND_ASSIGNMENT_MARKER}{value}");
        }
        if !quoted && !compound_assignment {
            if let Some(expanded) = self.expand_fast_assignment_value(value) {
                return expanded;
            }
        }
        self.apply_parameter_assignment_expansions_in_word(value);
        if let Some(expanded) = self.expand_compound_positional_at_assignment(value) {
            if compound_assignment {
                return format!("{COMPOUND_ASSIGNMENT_MARKER}{expanded}");
            }
            return expanded;
        }
        if let Some(expanded) = self.expand_unquoted_parameter_compound_assignment(value) {
            if compound_assignment {
                return format!("{COMPOUND_ASSIGNMENT_MARKER}{expanded}");
            }
            return expanded;
        }

        if !compound_assignment && !value.starts_with("$((") && !value.starts_with("$[") {
            if let Some(source) = value
                .strip_prefix("$(")
                .and_then(|rest| rest.strip_suffix(')'))
                // GNU expands each $() span in the RHS separately ("$(a)$(b)"
                // concatenates two substitution outputs, subst.c string
                // extraction never spans across substitutions). Keep the
                // single-substitution fast path only for words that are
                // exactly one $() group; multi-span values fall through to
                // expand_mixed_command_substitution_assignment (issue
                // niubash#71).
                .filter(|_| command_substitution_spans_whole_word(value))
            {
                let result = self.expand_command_substitution_mut_typed_with_context(
                    source,
                    if quoted {
                        SubstitutionQuoteContext::DoubleQuoted
                    } else {
                        SubstitutionQuoteContext::Unquoted
                    },
                );
                return result.assignment_text();
            }
        }

        if !compound_assignment {
            if let Some(output) = self.expand_backtick_substitution_typed(value, quoted) {
                return output.assignment_text();
            }
            if let Some(separator) = value.find('=') {
                let (prefix, rhs) = value.split_at(separator);
                if is_shell_name(prefix) {
                    if let Some(output) = self.expand_backtick_substitution_typed(&rhs[1..], quoted)
                    {
                        return format!("{prefix}={}", output.assignment_text());
                    }
                }
            }
            if let Some(expanded) = self.expand_mixed_command_substitution_assignment(value) {
                return expanded;
            }
        }

        if let Some(expanded) = self.expand_backtick_substitution(value) {
            return expanded;
        }

        let expanded_value = self.expand_embedded_parameters_mut(value);
        let expanded = if quoted {
            // Prompt transforms consume Bash's `\!` and `\#` escapes after
            // parameter expansion. Keep those two quoted backslashes until
            // `${var@P}` reaches prompt_expansion; ordinary shell escapes
            // still undergo the normal assignment quote-removal pass.
            {
                let mut restored = preserve_prompt_escapes(&expanded_value).replace('\x11', "");
                if value.contains(['\x16', '\x17', '\x18']) {
                    restored = restored
                        .replace('\x16', "'")
                        .replace('\x17', "'")
                        .replace('\x18', "\"")
                        .replace("\\'", "'");
                }
                restored
            }
        } else {
            // GNU strips quote syntax that parameter expansion introduced into
            // an unquoted assignment RHS (`v=${IFS+'}'z}` stores `}z`). Quotes
            // inside protected substitution payloads are data, so leave those
            // values alone. Escaped-quote markers (\x17 from \' and \x18 from
            // \" in the source word) are DATA quotes: parse.y records a
            // backslash-escaped quote as a quoted literal that survives quote
            // removal into the stored value (`x=a\'b` stores `a'b`). Hoist the
            // markers out of the quote-removal pass so the data quotes they
            // become are not re-stripped as syntax, then restore them.
            const DATA_SINGLE_QUOTE: &str = "\u{E000}";
            const DATA_DOUBLE_QUOTE: &str = "\u{E001}";
            let hoisted_value = value
                .replace('\x17', DATA_SINGLE_QUOTE)
                .replace('\x18', DATA_DOUBLE_QUOTE);
            let expanded_value = self.expand_embedded_parameters_mut(&hoisted_value);
            let stripped = if expanded_value.contains(['\'', '"'])
                && !contains_command_substitution_payload(&expanded_value)
            {
                crate::lexer::remove_shell_quotes(&expanded_value)
            } else {
                expanded_value.clone()
            };
            unescape_remaining_shell_escapes(&stripped)
                .replace(DATA_SINGLE_QUOTE, "'")
                .replace(DATA_DOUBLE_QUOTE, "\"")
        };
        let mut expanded = decode_command_substitution_payload(&expanded);
        if expanded.contains("<(") || expanded.contains(">(") {
            if let Ok(materialized) = self.materialize_assignment_process_substitutions(&expanded) {
                expanded = materialized;
            }
        }
        if value.starts_with('(') && value.ends_with(')') {
            if compound_assignment {
                return format!("{COMPOUND_ASSIGNMENT_MARKER}{expanded}");
            }
            return expanded;
        }
        if value.contains('=') {
            return expanded;
        }

        if quoted {
            return expanded;
        }

        // TODO(subst.c/variables.c): Bash's assignment-word expansion has a
        // special tilde pass on RHS prefixes and selected colon-separated
        // path positions. Keep it centralized here until Rubash ports the
        // `expand_string_assignment`/SHELL_VAR path more directly.
        self.expand_assignment_tilde(&expanded)
    }

    fn expand_mixed_command_substitution_assignment(&mut self, value: &str) -> Option<String> {
        let spans = scan_substitution_spans(value);
        if spans.is_empty() {
            return None;
        }
        let mut word = ExpandedWord::default();
        let mut cursor = 0usize;
        for span in spans {
            let raw = value.get(span.start..span.end)?;
            let prefix = self.expand_embedded_parameters_mut(value.get(cursor..span.start)?);
            word.append_literal(&prefix, true);
            let output = if let Some(source) = raw
                .strip_prefix("$(")
                .and_then(|rest| rest.strip_suffix(')'))
            {
                self.expand_command_substitution_mut_typed_with_context(source, span.context)
            } else if raw.starts_with('`') {
                self.expand_backtick_substitution_typed(
                    raw,
                    matches!(span.context, SubstitutionQuoteContext::DoubleQuoted),
                )?
            } else {
                return None;
            };
            word.append_substitution(output);
            cursor = span.end;
        }
        let suffix = self.expand_embedded_parameters_mut(value.get(cursor..)?);
        word.append_literal(&suffix, true);
        self.last_command_substitution_status.set(word.status);
        Some(word.materialize_lossy_at_boundary())
    }

    fn expand_fast_assignment_value(&mut self, value: &str) -> Option<String> {
        if let Some(expression) = value
            .strip_prefix("$((")
            .and_then(|rest| rest.strip_suffix("))"))
            .filter(|expression| !expression.contains("${"))
        {
            let Some(value) = self.eval_arithmetic_command_value(expression) else {
                // GNU expr.c raises evalerror from the actual evaluation, so
                // the recorded real-environment category decides fatality.
                // A fresh-environment re-evaluation would lose state-dependent
                // errors like `x+=2` on a declared integer, and a `set -u`
                // unbound variable must stay fatal even though a fresh
                // environment would happily evaluate it as 0.
                let actual_fatal = self.arithmetic_last_error_category.take().is_some()
                    || self.arithmetic_nounset_error.get();
                if !actual_fatal
                    && !crate::executor::arithmetic::arithmetic_expansion_is_fatal(expression)
                {
                    self.arithmetic_nonfatal_error.set(true);
                }
                if self.arithmetic_nounset_error.get() {
                    // `set -u` unbound is script-fatal (command_prepare turns
                    // the recorded flag into ExitCode). Returning an empty
                    // value here stops the slower assignment expanders from
                    // re-processing the `$(( ))` text as a command
                    // substitution, which produced a spurious
                    // `b: command not found` (issue #67).
                    return Some(String::new());
                }
                return None;
            };
            return Some(self.expand_assignment_tilde_if_needed(value.to_string()));
        }

        let parameter = value.strip_prefix('$')?;
        if parameter.len() != 1 {
            return None;
        }

        let expanded = match parameter.as_bytes()[0] {
            b'0' => self.script_name_value(),
            b'1'..=b'9' => {
                let index = usize::from(parameter.as_bytes()[0] - b'0' - 1);
                self.positional_params
                    .get(index)
                    .cloned()
                    .unwrap_or_default()
            }
            b'@' | b'*' => self.positional_params.join(" "),
            b'#' => self.positional_params.len().to_string(),
            b'?' => self.exit_code.to_string(),
            b'$' => self.shell_pid_value().to_string(),
            b'!' => self.last_background_pid_value(),
            b'-' => self.shell_option_flags(),
            _ => return None,
        };
        Some(self.expand_assignment_tilde_if_needed(expanded))
    }

    fn expand_assignment_tilde_if_needed(&self, value: String) -> String {
        if value.contains('=')
            || !tilde_expand::assignment_value_needs_tilde_expansion(&value, true)
            || (self.env_vars.get("__RUBASH_POSIX_MODE").map(String::as_str) == Some("1")
                && !value.starts_with("~/"))
        {
            return value;
        }

        self.expand_assignment_tilde(&value)
    }

    fn expand_quoted_prompt_arithmetic_assignment(&mut self, value: &str) -> String {
        #[derive(Clone, Copy, PartialEq, Eq)]
        enum QuoteMode {
            None,
            Single,
            Double,
        }

        let mut output = String::with_capacity(value.len());
        let mut segment = String::new();
        let mut mode = QuoteMode::None;

        for ch in value.chars() {
            match (mode, ch) {
                (QuoteMode::None, '\'') => {
                    output.push_str(&self.expand_embedded_parameters_mut(&segment));
                    segment.clear();
                    mode = QuoteMode::Single;
                }
                (QuoteMode::None, '"') => {
                    output.push_str(&self.expand_embedded_parameters_mut(&segment));
                    segment.clear();
                    mode = QuoteMode::Double;
                }
                (QuoteMode::Single, '\'') => {
                    output.push_str(&segment);
                    segment.clear();
                    mode = QuoteMode::None;
                }
                (QuoteMode::Double, '"') => {
                    output.push_str(&self.expand_embedded_parameters_mut(&segment));
                    segment.clear();
                    mode = QuoteMode::None;
                }
                _ => segment.push(ch),
            }
        }

        if mode == QuoteMode::Single {
            output.push_str(&segment);
        } else {
            output.push_str(&self.expand_embedded_parameters_mut(&segment));
        }

        preserve_prompt_escapes(&output)
    }

    pub(in crate::executor) fn expand_compound_positional_at_assignment(
        &self,
        value: &str,
    ) -> Option<String> {
        let inner = value.strip_prefix('(')?.strip_suffix(')')?;
        let mut changed = false;
        let mut values = Vec::new();
        for token in split_storage_words(inner) {
            let token = unquote_storage_value(&token);
            if token.strip_prefix('\x1d') == Some("${@}") || token == "$@" {
                changed = true;
                values.extend(
                    self.positional_params
                        .iter()
                        .map(|value| quote_array_value(value)),
                );
            } else if let Some(array_name) = token
                .strip_prefix('\x1d')
                .and_then(|token| token.strip_prefix("${"))
                .and_then(|token| token.strip_suffix("[@]}"))
            {
                if let Some(storage) = self.parameter_array_storage(array_name) {
                    changed = true;
                    values.extend(
                        array_values(&storage)
                            .iter()
                            .map(|value| quote_array_value(value)),
                    );
                } else {
                    values.push(quote_array_value(""));
                }
            } else if let Some(name) = token
                .strip_prefix('\x1d')
                .and_then(|token| token.strip_prefix("${"))
                .and_then(|token| token.strip_suffix('}'))
            {
                if let Some((var_name, offset, length)) = self.parse_parameter_substring(name) {
                    if var_name == "@" {
                        changed = true;
                        values.extend(
                            positional_parameter_substring(&self.positional_params, offset, length)
                                .iter()
                                .map(|value| quote_array_value(value)),
                        );
                        continue;
                    }
                    if let Some(array_name) = var_name
                        .strip_suffix("[@]")
                        .or_else(|| var_name.strip_suffix("[*]"))
                    {
                        if let Some(storage) = self.parameter_array_storage(array_name) {
                            changed = true;
                            values.extend(
                                array_parameter_slice(
                                    &storage,
                                    offset,
                                    length.and_then(|length| usize::try_from(length).ok()),
                                )
                                .iter()
                                .map(|value| quote_array_value(value)),
                            );
                            continue;
                        }
                    }
                }
                values.push(quote_array_value(&token));
            } else {
                values.push(quote_array_value(&token));
            }
        }
        changed.then(|| format!("({})", values.join(" ")))
    }

    pub(in crate::executor) fn expand_unquoted_parameter_compound_assignment(
        &self,
        value: &str,
    ) -> Option<String> {
        let inner = value.strip_prefix('(')?.strip_suffix(')')?.trim();
        let unquoted_inner = strip_matching_quotes(inner);
        let parameter = if unquoted_inner == inner {
            inner
        } else {
            &unquoted_inner
        };
        let value = if let Some(name) = single_unquoted_parameter_name(parameter) {
            self.shell_variable_value(name).unwrap_or_default()
        } else if let Some(name) = parameter
            .strip_prefix("${")
            .and_then(|name| name.strip_suffix('}'))
        {
            let name = name.replace("\\\"", "\"").replace("\\'", "'");
            self.array_element_parameter_value(&name)?
        } else {
            return None;
        };
        let values =
            field_split_values_with_ifs(&value, self.env_vars.get("IFS").map(String::as_str))
                .into_iter()
                .map(|value| {
                    format!(
                        "{ARRAY_FIELD_SPLIT_MARKER}{}",
                        quote_compound_field_value(&value)
                    )
                })
                .collect::<Vec<_>>();
        Some(format!("({})", values.join(" ")))
    }

    pub(in crate::executor) fn expand_quoted_array_assignment_value(
        &self,
        value: &str,
    ) -> Option<String> {
        let value = value.strip_prefix('\x1d').unwrap_or(value);
        let name = value.strip_prefix("${")?.strip_suffix('}')?;
        let array_name = name
            .strip_suffix("[@]")
            .or_else(|| name.strip_suffix("[*]"))
            .filter(|array_name| is_shell_name(array_name))?;
        self.parameter_array_storage(array_name)
            .map(|value| self.join_array_parameter_values(&value, name))
    }

    pub(in crate::executor) fn expand_assignment_value_with_status(
        &mut self,
        value: &str,
    ) -> (String, Option<i32>) {
        let result = self.expand_assignment_value_result(value);
        (result.value, result.substitution_status)
    }
}

fn preserve_prompt_escapes(value: &str) -> String {
    const PROTECTED_PROMPT_ESCAPE: char = '\x15';
    let mut preserved = String::with_capacity(value.len());
    let mut chars = value.chars().peekable();
    while let Some(ch) = chars.next() {
        if ch == '\\' && matches!(chars.peek(), Some('!' | '#')) {
            preserved.push(PROTECTED_PROMPT_ESCAPE);
            preserved.push(chars.next().expect("peeked prompt escape"));
        } else {
            preserved.push(ch);
        }
    }
    preserved.replace(PROTECTED_PROMPT_ESCAPE, "\\")
}
