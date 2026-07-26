use super::*;

impl Executor {
    pub(in crate::executor) fn expand_assignment_value(&mut self, value: &str) -> String {
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

        if let Some(expanded) = self.expand_backtick_substitution(value) {
            return expanded;
        }

        let expanded = self.expand_embedded_parameters_mut(value);
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

    fn expand_fast_assignment_value(&mut self, value: &str) -> Option<String> {
        if let Some(expression) = value
            .strip_prefix("$((")
            .and_then(|rest| rest.strip_suffix("))"))
            .filter(|expression| !expression.contains("${"))
        {
            let value = self.eval_arithmetic_command_value(expression)?.to_string();
            return Some(self.expand_assignment_tilde_if_needed(value));
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
            b'$' => std::process::id().to_string(),
            b'!' => self.last_background_pid_value(),
            b'-' => self.shell_option_flags(),
            _ => return None,
        };
        Some(self.expand_assignment_tilde_if_needed(expanded))
    }

    fn expand_assignment_tilde_if_needed(&self, value: String) -> String {
        if value.contains('=')
            || (self.env_vars.get("__RUBASH_POSIX_MODE").map(String::as_str) == Some("1")
                && !value.starts_with("~/"))
        {
            return value;
        }

        self.expand_assignment_tilde(&value)
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
        let name = single_unquoted_parameter_name(inner)?;
        let value = self.shell_variable_value(name).unwrap_or_default();
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
        self.last_command_substitution_status.set(None);
        let expanded = self.expand_assignment_value(value);
        let status = self.last_command_substitution_status.get();
        self.last_command_substitution_status.set(None);
        (expanded, status)
    }

    pub(in crate::executor) fn do_env(&mut self) {
        for (key, value) in &self.env_vars {
            println!("{}={}", key, value);
        }
        self.exit_code = 0;
    }
}
