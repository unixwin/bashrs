//! Array-related functions for the executor module.
//!
//! Contains free functions and `Executor` methods for working with
//! indexed arrays, array storage, and array subscripts.

mod executor;
mod mapfile;
mod storage;

pub(super) use mapfile::split_mapfile_input;
pub(super) use storage::{
    array_indices, array_value_at, array_values, format_indexed_array_storage,
    format_indexed_array_values, indexed_array_entries, is_array_storage, is_marked_array_var,
    normalize_array_expanded_value, parse_array_integer_subscript, parse_array_numeric_subscript,
    parse_array_subscript, quote_array_value, resolve_indexed_array_subscript, store_indexed_array,
};

use std::collections::{BTreeMap, HashMap};
use std::env;
use std::fs;

use super::{
    apply_parameter_case_mod, assoc_value_at, case_pattern_matches,
    eval_arith_value, eval_conditional_arith_value,
    is_marked_var, is_shell_name, parse_indirect_pattern_removal, parse_parameter_case_mod,
    parse_parameter_replacement, parse_parameter_transform, pattern_contains_glob, quote_assoc_key,
    remove_parameter_pattern, split_storage_words,
    strip_matching_quotes, unquote_storage_value, Executor, ParameterTransform,
    ARRAY_FIELD_SPLIT_MARKER, ASSOC_VARS,
};

pub(super) fn is_array_element_assignment_word(word: &str) -> bool {
    let Some((left, _)) = word.split_once('=') else {
        return false;
    };
    let left = left.strip_suffix('+').unwrap_or(left);
    let Some((name, index)) = left.split_once('[') else {
        return false;
    };
    is_shell_name(name) && index.ends_with(']')
}

pub(super) fn append_scalar_value(current: &str, value: &str) -> String {
    let mut output = current.to_string();
    output.push_str(value);
    output
}

fn is_ifs_whitespace(ch: char) -> bool {
    matches!(ch, ' ' | '\t' | '\n')
}

fn split_ifs_whitespace(value: &str, ifs: &str) -> Vec<String> {
    let mut fields = Vec::new();
    let mut current = String::new();
    for ch in value.chars() {
        if ifs.contains(ch) {
            if !current.is_empty() {
                fields.push(std::mem::take(&mut current));
            }
        } else {
            current.push(ch);
        }
    }
    if !current.is_empty() {
        fields.push(current);
    }
    fields
}

fn split_mixed_ifs(value: &str, ifs: &str) -> Vec<String> {
    let chars = value.chars().collect::<Vec<_>>();
    let mut fields = Vec::new();
    let mut current = String::new();
    let mut index = 0;

    while index < chars.len() {
        let ch = chars[index];
        if !ifs.contains(ch) {
            current.push(ch);
            index += 1;
            continue;
        }

        if is_ifs_whitespace(ch) {
            while index < chars.len()
                && ifs.contains(chars[index])
                && is_ifs_whitespace(chars[index])
            {
                index += 1;
            }
            if index < chars.len() && !is_ifs_whitespace(chars[index]) && ifs.contains(chars[index])
            {
                continue;
            }
            if !current.is_empty() {
                fields.push(std::mem::take(&mut current));
            }
            continue;
        }

        fields.push(std::mem::take(&mut current));
        index += 1;
        while index < chars.len() && ifs.contains(chars[index]) && is_ifs_whitespace(chars[index]) {
            index += 1;
        }
    }

    if !current.is_empty() {
        fields.push(current);
    }
    fields
}

pub(super) fn field_split_values_with_ifs(value: &str, ifs: Option<&str>) -> Vec<String> {
    let Some(ifs) = ifs else {
        return split_ifs_whitespace(value, " \t\n");
    };
    if ifs.is_empty() {
        return vec![value.to_string()];
    }

    // Bash defines IFS whitespace narrowly as space, tab, and newline.
    // Other Unicode whitespace remains data unless explicitly listed in IFS.
    if ifs == " \t\n" {
        return split_ifs_whitespace(value, ifs);
    }

    if ifs.chars().all(is_ifs_whitespace) {
        return split_ifs_whitespace(value, ifs);
    }

    if ifs.chars().any(is_ifs_whitespace) {
        return split_mixed_ifs(value, ifs);
    }

    // Non-whitespace IFS: every separator produces a field. Bash keeps leading
    // and internal empty fields (`IFS=:; set -- :a::b`) and drops only the
    // final empty field produced by a trailing delimiter.
    let mut fields: Vec<String> = value
        .split(|ch| ifs.contains(ch))
        .map(str::to_string)
        .collect();
    if fields.last().is_some_and(|field| field.is_empty()) && fields.len() > 1 {
        fields.pop();
    }
    fields
}

pub(super) fn field_split_array_values_with_ifs(
    values: Vec<String>,
    ifs: Option<&str>,
) -> Vec<String> {
    values
        .into_iter()
        .flat_map(|value| field_split_values_with_ifs(&value, ifs))
        .collect()
}

pub(super) fn field_split_positional_values_with_ifs(
    values: Vec<String>,
    ifs: Option<&str>,
) -> Vec<String> {
    let value_count = values.len();
    values
        .into_iter()
        .enumerate()
        .flat_map(|(index, value)| {
            let is_last = index + 1 == value_count;
            let mut fields = if value.is_empty() {
                if is_last {
                    Vec::new()
                } else {
                    vec![value]
                }
            } else if let Some(ifs) = ifs.filter(|ifs| ifs.chars().any(|ch| !ch.is_whitespace())) {
                if ifs.chars().any(is_ifs_whitespace) {
                    split_mixed_ifs(&value, ifs)
                } else {
                    value
                        .split(|ch| ifs.contains(ch))
                        .map(str::to_string)
                        .collect()
                }
            } else {
                field_split_values_with_ifs(&value, ifs)
            };
            if is_last {
                while fields.last().is_some_and(|field| field.is_empty()) {
                    fields.pop();
                }
            }
            fields
        })
        .collect()
}

#[cfg(test)]
mod field_split_tests {
    use super::field_split_values_with_ifs;

    #[test]
    fn non_whitespace_ifs_keeps_leading_empty_fields() {
        assert_eq!(field_split_values_with_ifs(":", Some(":")), vec![""]);
        assert_eq!(field_split_values_with_ifs("::", Some(":")), vec!["", ""]);
        assert_eq!(field_split_values_with_ifs(":a:", Some(":")), vec!["", "a"]);
    }

    #[test]
    fn mixed_ifs_keeps_leading_empty_fields() {
        assert_eq!(field_split_values_with_ifs(" : ", Some(": ")), vec![""]);
        assert_eq!(field_split_values_with_ifs(": :", Some(": ")), vec!["", ""]);
    }

    #[test]
    fn custom_newline_ifs_preserves_spaces_inside_fields() {
        assert_eq!(
            field_split_values_with_ifs("a b\na c\nx z", Some("\n")),
            vec!["a b", "a c", "x z"]
        );
    }

    #[test]
    fn default_ifs_still_splits_shell_whitespace() {
        assert_eq!(
            field_split_values_with_ifs("a b\na c", Some(" \t\n")),
            vec!["a", "b", "a", "c"]
        );
    }

    #[test]
    fn custom_space_ifs_collapses_repeated_spaces() {
        assert_eq!(
            field_split_values_with_ifs("a  b", Some(" ")),
            vec!["a", "b"]
        );
    }

    #[test]
    fn custom_newline_ifs_keeps_spaces_in_fields() {
        assert_eq!(
            field_split_values_with_ifs("a  b\nc  d", Some("\n")),
            vec!["a  b", "c  d"]
        );
    }

    #[test]
    fn default_ifs_does_not_split_vertical_tab_or_form_feed() {
        assert_eq!(
            field_split_values_with_ifs("a\x0bb\x0cc", None),
            vec!["a\x0bb\x0cc"]
        );
        assert_eq!(
            field_split_values_with_ifs("a\x0bb\x0cc", Some(" \t\n")),
            vec!["a\x0bb\x0cc"]
        );
    }

    #[test]
    fn default_ifs_does_not_split_non_ascii_whitespace() {
        assert_eq!(
            field_split_values_with_ifs("a\u{00a0}b\u{2003}c", None),
            vec!["a\u{00a0}b\u{2003}c"]
        );
    }
}

pub(super) fn word_is_unquoted_array_list_expansion(word: &str) -> bool {
    if word.starts_with('"') || word.starts_with('\'') || word.starts_with('\x1d') {
        return false;
    }

    let Some(inner) = word
        .strip_prefix("${")
        .and_then(|word| word.strip_suffix('}'))
    else {
        return false;
    };
    let name = inner.split_once(':').map_or(inner, |(name, _)| name);
    let name = parse_parameter_transform(name)
        .map(|(name, _)| name)
        .or_else(|| parse_indirect_pattern_removal(name).map(|(name, _, _)| name))
        .or_else(|| parse_parameter_replacement(name).map(|(name, _, _, _)| name))
        .or_else(|| parse_parameter_case_mod(name).map(|(name, _, _)| name))
        .unwrap_or(name);
    name.ends_with("[@]") || name.ends_with("[*]")
}

pub(super) fn pathname_expand_array_token(token: &str) -> Option<Vec<String>> {
    if token.starts_with('"') || token.starts_with('\'') || !pattern_contains_glob(token) {
        return None;
    }
    if token.contains('/') || token.contains('\\') {
        return None;
    }
    let include_dotfiles = token.starts_with('.');
    let mut matches = fs::read_dir(env::current_dir().ok()?)
        .ok()?
        .filter_map(Result::ok)
        .filter_map(|entry| entry.file_name().into_string().ok())
        .filter(|name| include_dotfiles || !name.starts_with('.'))
        .filter(|name| case_pattern_matches(token, name))
        .collect::<Vec<_>>();
    if matches.is_empty() {
        return None;
    }
    matches.sort();
    Some(matches)
}

pub(super) fn append_array_value(
    current: &str,
    value: &str,
    integer: bool,
    ifs: Option<&str>,
    env_vars: &HashMap<String, String>,
) -> String {
    let mut entries = indexed_array_entries(current);
    let mut next_index = entries
        .keys()
        .next_back()
        .map(|index| index + 1)
        .unwrap_or(0);
    // Bash appends `arr+=str` (no parens) to arr[0] for any array, not just
    // integer arrays; `arr+=(x y)` appends new elements.
    let scalar_append = !value.starts_with('(');
    let brace_expand = crate::builtins::set::shell_option_enabled(env_vars, "braceexpand");
    let tokens = array_assignment_tokens(value)
        .into_iter()
        .flat_map(|token| {
            if brace_expand && !token.contains("${") && !token.contains('=') {
                crate::expand::braces::expand_braces(&token)
            } else {
                vec![token]
            }
        })
        .collect::<Vec<_>>();
    for token in tokens {
        if let Some(matches) = pathname_expand_array_token(&token) {
            for value in matches {
                entries.insert(next_index, value);
                next_index += 1;
            }
            continue;
        }

        if let Some((left, rhs)) = token.split_once("+=") {
            if let Some(index) = array_assignment_index(left, &entries, env_vars) {
                let current = entries.get(&index).cloned().unwrap_or_default();
                let rhs = unquote_storage_value(rhs);
                let value = if integer {
                    (eval_arith_value(&current) + eval_arith_value(&rhs)).to_string()
                } else {
                    append_scalar_value(&current, &rhs)
                };
                entries.insert(index, value);
                next_index = index + 1;
                continue;
            }
            if array_assignment_has_subscript(left) {
                continue;
            }
        }

        if let Some((left, rhs)) = token.split_once('=') {
            if let Some(index) = array_assignment_index(left, &entries, env_vars) {
                entries.insert(index, unquote_storage_value(rhs));
                next_index = index + 1;
                continue;
            }
            if array_assignment_has_subscript(left) {
                continue;
            }
        }

        let command_subst_token = token.starts_with("\"$(") && token.ends_with('"');
        let quoted_token = token.starts_with('"') && token.ends_with('"') && !command_subst_token;
        if let Some(token) = token.strip_prefix(ARRAY_FIELD_SPLIT_MARKER) {
            let token = unquote_storage_value(token);
            if let Some(matches) = pathname_expand_array_token(&token) {
                for value in matches {
                    entries.insert(next_index, value);
                    next_index += 1;
                }
            } else {
                entries.insert(next_index, token);
                next_index += 1;
            }
            continue;
        }
        let token = unquote_storage_value(&token);
        if let Some(expanded_array) = token.strip_prefix('\x1d') {
            for value in field_split_values_with_ifs(expanded_array, ifs) {
                if let Some(matches) = pathname_expand_array_token(&value) {
                    for value in matches {
                        entries.insert(next_index, value);
                        next_index += 1;
                    }
                } else {
                    entries.insert(next_index, value.to_string());
                    next_index += 1;
                }
            }
            continue;
        }
        if token.contains('\n') || (token.contains(char::is_whitespace) && !quoted_token) {
            for value in field_split_values_with_ifs(&token, ifs) {
                entries.insert(next_index, value.to_string());
                next_index += 1;
            }
            continue;
        }
        if scalar_append && !entries.is_empty() {
            let current = entries.get(&0).cloned().unwrap_or_default();
            let appended = if integer {
                (eval_arith_value(&current) + eval_arith_value(&token)).to_string()
            } else {
                append_scalar_value(&current, &token)
            };
            entries.insert(0, appended);
        } else {
            entries.insert(next_index, token);
            next_index += 1;
        }
    }

    if integer {
        for element in entries.values_mut() {
            *element = eval_arith_value(element).to_string();
        }
    }

    format_indexed_array_storage(entries)
}

pub(super) fn array_assignment_index(
    left: &str,
    entries: &BTreeMap<usize, String>,
    env_vars: &HashMap<String, String>,
) -> Option<usize> {
    let expression = left.strip_prefix('[')?.strip_suffix(']')?;
    let index = eval_conditional_arith_value(expression, env_vars)?;
    if index >= 0 {
        return usize::try_from(index).ok();
    }
    let max_index = entries.keys().next_back().copied()?;
    let resolved = i128::try_from(max_index)
        .ok()?
        .checked_add(1)?
        .checked_add(index)?;
    usize::try_from(resolved).ok()
}

pub(super) fn array_assignment_has_subscript(left: &str) -> bool {
    left.strip_prefix('[')
        .and_then(|value| value.strip_suffix(']'))
        .is_some()
}

pub(super) fn array_assignment_tokens(value: &str) -> Vec<String> {
    let Some(inner) = value
        .strip_prefix('(')
        .and_then(|value| value.strip_suffix(')'))
    else {
        return if value.is_empty() {
            Vec::new()
        } else {
            vec![value.to_string()]
        };
    };

    split_storage_words(inner).collect()
}

pub(super) fn array_parameter_slice(
    value: &str,
    offset: isize,
    length: Option<usize>,
) -> Vec<String> {
    let values = array_values(value);
    let start = if offset < 0 {
        values.len().saturating_sub(offset.unsigned_abs())
    } else {
        offset as usize
    };

    values
        .into_iter()
        .skip(start)
        .take(length.unwrap_or(usize::MAX))
        .collect()
}

pub(super) fn slice_array_values(
    values: Vec<String>,
    offset: isize,
    length: Option<usize>,
) -> Vec<String> {
    let start = if offset < 0 {
        values.len().saturating_sub(offset.unsigned_abs())
    } else {
        offset as usize
    };

    values
        .into_iter()
        .skip(start)
        .take(length.unwrap_or(usize::MAX))
        .collect()
}

pub(super) fn is_noassign_bash_array(name: &str) -> bool {
    matches!(
        name,
        "BASH_ARGC" | "BASH_ARGV" | "BASH_LINENO" | "BASH_SOURCE" | "FUNCNAME" | "PIPESTATUS"
    )
}
