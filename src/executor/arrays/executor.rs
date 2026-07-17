use super::*;
use crate::executor::assoc_keys;

impl Executor {
    pub(in crate::executor) fn indexed_array_stack(&self, name: &str) -> Vec<String> {
        self.env_vars
            .get(name)
            .map(|value| array_values(value))
            .unwrap_or_default()
    }

    pub(in crate::executor) fn array_assignment_transform(&self, name: &str) -> String {
        let Some(value) = self.env_vars.get(name) else {
            return String::new();
        };

        if is_marked_var(&self.env_vars, ASSOC_VARS, name) {
            let entries = assoc_entries(value);
            if entries.is_empty() {
                return format!("declare -A {name}");
            }
            let rendered = entries
                .into_iter()
                .map(|(key, value)| {
                    format!("[{}]={}", quote_assoc_key(&key), quote_array_value(&value))
                })
                .collect::<Vec<_>>()
                .join(" ");
            return format!("declare -A {name}=({rendered} )");
        }

        if is_marked_array_var(&self.env_vars, name) || is_array_storage(value) {
            let rendered = indexed_array_entries(value)
                .into_iter()
                .map(|(index, value)| format!("[{index}]={}", quote_array_value(&value)))
                .collect::<Vec<_>>()
                .join(" ");
            return format!("declare -a {name}=({rendered})");
        }

        String::new()
    }

    pub(in crate::executor) fn array_element_parameter_value(
        &self,
        expression: &str,
    ) -> Option<String> {
        let (array_name, key) = parse_array_subscript(expression)?;
        let storage_name = self.resolved_variable_name(array_name)?;
        let storage = self.parameter_array_storage(array_name)?;
        if is_marked_var(&self.env_vars, ASSOC_VARS, &storage_name) {
            let key = self.assoc_subscript_key(key);
            return assoc_value_at(&storage, &key);
        }
        let key = strip_matching_quotes(&self.expand_embedded_parameters(key)).to_string();
        eval_conditional_arith_value(&key, &self.env_vars)
            .and_then(|index| resolve_indexed_array_subscript(&storage, index))
            .and_then(|index| array_value_at(&storage, index))
    }

    pub(in crate::executor) fn array_length(&self, name: &str) -> usize {
        if name == "GROUPS" {
            return self.groups_words().len();
        }
        self.parameter_array_storage(name)
            .map(|value| array_values(&value).len())
            .unwrap_or(0)
    }

    pub(in crate::executor) fn array_at_word_values(&self, word: &str) -> Option<Vec<String>> {
        let quoted_array_word =
            (word.starts_with('"') && word.ends_with('"')) || word.starts_with('\x1d');
        let word = word
            .strip_prefix('"')
            .and_then(|word| word.strip_suffix('"'))
            .unwrap_or(word);
        let word = word.strip_prefix('\x1d').unwrap_or(word);
        if let Some(values) = self.array_transform_word_values(word, quoted_array_word) {
            return Some(values);
        }
        if let Some(values) = self.array_pattern_word_values(word, quoted_array_word) {
            return Some(values);
        }
        if !quoted_array_word {
            if let Some((name, offset, length)) = word
                .strip_prefix("${")
                .and_then(|word| word.strip_suffix('}'))
                .and_then(|name| self.parse_parameter_substring(name))
            {
                if let Some(array_name) = name
                    .strip_suffix("[@]")
                    .or_else(|| name.strip_suffix("[*]"))
                {
                    return self.parameter_array_storage(array_name).map(|value| {
                        array_parameter_slice(
                            &value,
                            offset,
                            length.and_then(|length| usize::try_from(length).ok()),
                        )
                    });
                }
            }
        }
        if quoted_array_word {
            if let Some((name, offset, length)) = word
                .strip_prefix("${")
                .and_then(|word| word.strip_suffix('}'))
                .and_then(|name| self.parse_parameter_substring(name))
            {
                if let Some(indirect_name) = name.strip_prefix('!') {
                    let target_expr = self.env_vars.get(indirect_name)?;
                    let expands_as_array = target_expr.ends_with("[@]")
                        || (!quoted_array_word && target_expr.ends_with("[*]"));
                    if expands_as_array {
                        return Some(slice_array_values(
                            self.indirect_target_values(target_expr),
                            offset,
                            length.and_then(|length| usize::try_from(length).ok()),
                        ));
                    }
                }
                if let Some(array_name) = name.strip_suffix("[@]") {
                    if array_name == "GROUPS" {
                        return Some(slice_array_values(
                            self.groups_words(),
                            offset,
                            length.and_then(|length| usize::try_from(length).ok()),
                        ));
                    }
                    return self.parameter_array_storage(array_name).map(|value| {
                        array_parameter_slice(
                            &value,
                            offset,
                            length.and_then(|length| usize::try_from(length).ok()),
                        )
                    });
                }
            }
            if let Some(values) = self.indirect_array_reference_word_values(word, true) {
                return Some(values);
            }
            if let Some(prefix) = word
                .strip_prefix("${!")
                .and_then(|word| word.strip_suffix("@}"))
            {
                let mut names = self
                    .env_vars
                    .keys()
                    .map(String::as_str)
                    .filter(|name| name.starts_with(prefix))
                    .map(str::to_string)
                    .collect::<Vec<_>>();
                names.sort_unstable();
                return Some(names);
            }
            if let Some(prefix) = word
                .strip_prefix("${!")
                .and_then(|word| word.strip_suffix("*}"))
            {
                let mut names = self
                    .env_vars
                    .keys()
                    .map(String::as_str)
                    .filter(|name| name.starts_with(prefix))
                    .map(str::to_string)
                    .collect::<Vec<_>>();
                names.sort_unstable();
                return Some(vec![names.join(&self.ifs_first_char_separator())]);
            }
            if let Some(name) = word
                .strip_prefix("${!")
                .and_then(|word| word.strip_suffix("[@]}"))
            {
                let storage_name = self.resolved_variable_name(name)?;
                let storage = self.parameter_array_storage(name)?;
                if is_marked_var(&self.env_vars, ASSOC_VARS, &storage_name) {
                    return Some(assoc_keys(&storage));
                }
                return Some(array_indices(&storage));
            }
            if let Some(name) = word
                .strip_prefix("${!")
                .and_then(|word| word.strip_suffix("[*]}"))
            {
                let storage_name = self.resolved_variable_name(name)?;
                let storage = self.parameter_array_storage(name)?;
                let keys = if is_marked_var(&self.env_vars, ASSOC_VARS, &storage_name) {
                    assoc_keys(&storage)
                } else {
                    array_indices(&storage)
                };
                return Some(vec![keys.join(&self.ifs_first_char_separator())]);
            }
        }
        if let Some(values) = self.indirect_array_reference_word_values(word, quoted_array_word) {
            return Some(values);
        }
        if quoted_array_word && word == "${GROUPS[*]}" {
            return Some(vec![self
                .groups_words()
                .join(&self.ifs_first_char_separator())]);
        }
        let name = word.strip_prefix("${").and_then(|word| {
            if quoted_array_word {
                word.strip_suffix("[@]}")
            } else {
                word.strip_suffix("[@]}")
                    .or_else(|| word.strip_suffix("[*]}"))
            }
        })?;
        if name == "GROUPS" {
            return Some(self.groups_words());
        }
        self.parameter_array_storage(name)
            .map(|value| array_values(&value))
    }

    fn array_transform_word_values(
        &self,
        word: &str,
        quoted_array_word: bool,
    ) -> Option<Vec<String>> {
        let (var_name, transform) = word
            .strip_prefix("${")
            .and_then(|word| word.strip_suffix('}'))
            .and_then(parse_parameter_transform)?;
        let (array_name, starred) = var_name
            .strip_suffix("[@]")
            .map(|name| (name, false))
            .or_else(|| var_name.strip_suffix("[*]").map(|name| (name, true)))?;
        if transform == ParameterTransform::Assignment {
            let value = self.parameter_assignment_transform(var_name);
            if quoted_array_word {
                return Some(split_array_assignment_transform_words(&value));
            }
            return Some(vec![value]);
        }
        if transform == ParameterTransform::KeyValueQuoted {
            return Some(vec![self.parameter_key_value_transform(var_name, true)]);
        }
        if transform == ParameterTransform::KeyValueSplit {
            return self.array_key_value_split_transform_values(array_name);
        }
        if !array_value_transform_splits_words(transform) {
            return None;
        }
        let storage = self.parameter_array_storage(array_name)?;
        let values = array_values(&storage)
            .into_iter()
            .map(|value| self.apply_parameter_transform_value(&value, transform))
            .collect::<Vec<_>>();
        if quoted_array_word && starred {
            return Some(vec![values.join(&self.ifs_first_char_separator())]);
        }
        Some(values)
    }

    fn array_pattern_word_values(
        &self,
        word: &str,
        quoted_array_word: bool,
    ) -> Option<Vec<String>> {
        let inner = word
            .strip_prefix("${")
            .and_then(|word| word.strip_suffix('}'))?;

        if let Some((var_name, pattern, operation)) = parse_indirect_pattern_removal(inner) {
            let pattern = self.expand_parameter_pattern_word(pattern);
            return self.array_modified_word_values(var_name, quoted_array_word, |value| {
                remove_parameter_pattern(value, &pattern, operation)
            });
        }

        if let Some((var_name, pattern, replacement, global)) = parse_parameter_replacement(inner) {
            let pattern = self.expand_parameter_pattern_word(pattern);
            let replacement = decode_parameter_replacement_quotes(
                &self.expand_embedded_parameters_preserving_escaped_single_quotes(replacement),
            );
            return self.array_modified_word_values(var_name, quoted_array_word, |value| {
                replace_parameter_pattern(value, &pattern, &replacement, global)
            });
        }

        if let Some((var_name, operation, pattern)) = parse_parameter_case_mod(inner) {
            let pattern = self.expand_embedded_parameters(pattern);
            return self.array_modified_word_values(var_name, quoted_array_word, |value| {
                apply_parameter_case_mod(value, operation, &pattern)
            });
        }

        None
    }

    fn array_modified_word_values<F>(
        &self,
        var_name: &str,
        quoted_array_word: bool,
        modify: F,
    ) -> Option<Vec<String>>
    where
        F: Fn(&str) -> String,
    {
        let (array_name, starred) = var_name
            .strip_suffix("[@]")
            .map(|name| (name, false))
            .or_else(|| var_name.strip_suffix("[*]").map(|name| (name, true)))?;
        let storage = self.parameter_array_storage(array_name)?;
        let values = array_values(&storage)
            .into_iter()
            .map(|value| modify(&value))
            .collect::<Vec<_>>();
        if quoted_array_word && starred {
            return Some(vec![values.join(&self.ifs_first_char_separator())]);
        }
        Some(values)
    }

    fn array_key_value_split_transform_values(&self, array_name: &str) -> Option<Vec<String>> {
        let storage_name = self.resolved_variable_name(array_name)?;
        let storage = self.parameter_array_storage(array_name)?;
        if is_marked_var(&self.env_vars, ASSOC_VARS, &storage_name) {
            return Some(
                assoc_entries(&storage)
                    .into_iter()
                    .flat_map(|(key, value)| [key, value])
                    .collect(),
            );
        }
        Some(
            indexed_array_entries(&storage)
                .into_iter()
                .flat_map(|(index, value)| [index.to_string(), value])
                .collect(),
        )
    }

    fn indirect_array_reference_word_values(
        &self,
        word: &str,
        quoted_array_word: bool,
    ) -> Option<Vec<String>> {
        let indirect_name = word
            .strip_prefix("${!")
            .and_then(|word| word.strip_suffix('}'))?;
        if !is_shell_name(indirect_name) {
            return None;
        }
        let target_expr = self.env_vars.get(indirect_name)?;
        if target_expr.ends_with("[@]") {
            return Some(self.indirect_target_values(target_expr));
        }
        if target_expr.ends_with("[*]") {
            let values = self.indirect_target_values(target_expr);
            if quoted_array_word {
                return Some(vec![values.join(&self.ifs_first_char_separator())]);
            }
            return Some(values);
        }
        None
    }

    fn ifs_first_char_separator(&self) -> String {
        self.env_vars
            .get("IFS")
            .and_then(|ifs| ifs.chars().next())
            .unwrap_or(' ')
            .to_string()
    }
}

fn array_value_transform_splits_words(transform: ParameterTransform) -> bool {
    matches!(
        transform,
        ParameterTransform::Quote
            | ParameterTransform::Escape
            | ParameterTransform::Prompt
            | ParameterTransform::Upper
            | ParameterTransform::UpperFirst
            | ParameterTransform::Lower
    )
}

fn split_array_assignment_transform_words(value: &str) -> Vec<String> {
    let mut parts = value.splitn(3, char::is_whitespace);
    let first = parts.next().unwrap_or_default();
    if first.is_empty() {
        return Vec::new();
    }
    let Some(second) = parts.next() else {
        return vec![first.to_string()];
    };
    let Some(rest) = parts.next() else {
        return vec![first.to_string(), second.to_string()];
    };
    vec![first.to_string(), second.to_string(), rest.to_string()]
}
