use super::*;

pub(in crate::executor) fn split_assignment_word(word: &str) -> Option<(&str, &str)> {
    let (name, value) = word.split_once('=')?;
    let (base_name, _) = assignment_name_and_append(name);
    if is_shell_name(base_name) {
        Some((name, value))
    } else {
        None
    }
}

pub(in crate::executor) fn assignment_name_and_append(name: &str) -> (&str, bool) {
    name.strip_suffix('+')
        .map(|base| (base, true))
        .unwrap_or((name, false))
}

pub(in crate::executor) fn arithmetic_expression_arg(expression: &str) -> String {
    expression.replace(COMPOUND_ASSIGNMENT_MARKER, "")
}

pub(in crate::executor) fn arithmetic_assignment_suffix(value: &str) -> bool {
    value
        .as_bytes()
        .first()
        .is_some_and(|ch| matches!(ch, b'+' | b'-' | b'*' | b'/' | b'%'))
}

pub(in crate::executor) fn single_unquoted_parameter_name(value: &str) -> Option<&str> {
    if let Some(name) = value
        .strip_prefix("${")
        .and_then(|name| name.strip_suffix('}'))
    {
        return is_shell_name(name).then_some(name);
    }
    let name = value.strip_prefix('$')?;
    is_shell_name(name).then_some(name)
}

pub(in crate::executor) fn append_assoc_value(current: &str, value: &str, integer: bool) -> String {
    // GNU arrayfunc.c assign_compound_array_list / bind_assoc_variable: when
    // the array carries the integer attribute, every element value is
    // evaluated as an arithmetic expression before it is stored
    // (assoc.tests: declare -i chaff; chaff=( [zero]=1+4 [one]=3+7 )).
    let eval_element = |raw: &str| -> String {
        if integer {
            super::arithmetic::eval_arith_value(raw).to_string()
        } else {
            raw.to_string()
        }
    };
    let mut entries = assoc_entries(current);
    let tokens = array_assignment_tokens(value);
    let explicit_subscripts = tokens
        .iter()
        .any(|token| assoc_assignment_token(token).is_some());

    if !explicit_subscripts {
        for pair in tokens.chunks(2) {
            let Some(key) = pair.first() else {
                continue;
            };
            let key = unquote_storage_value(key);
            let value = pair
                .get(1)
                .map(|value| unquote_storage_value(value))
                .unwrap_or_default();
            entries.push((key, eval_element(&value)));
        }
        return format_assoc_storage(entries);
    }

    for token in tokens {
        if let Some((key, rhs, append)) = assoc_assignment_token(&token) {
            let key = unquote_storage_value(key);
            let rhs = unquote_storage_value(rhs);
            if append {
                if let Some((_, entry_value)) = entries
                    .iter_mut()
                    .rev()
                    .find(|(entry_key, _)| entry_key == &key)
                {
                    *entry_value = append_scalar_value(entry_value, &rhs);
                    *entry_value = eval_element(entry_value);
                } else {
                    entries.push((key, eval_element(&rhs)));
                }
                continue;
            }
            entries.push((key, eval_element(&rhs)));
            continue;
        }
        // Bare element (no `[key]=` form): GNU rejects it with
        // "<name>: <word>: must use subscript when assigning associative
        // array" and skips the element. The error is emitted by the caller
        // (apply_shell_assignment) which owns the diagnostic context.
    }

    format_assoc_storage(entries)
}

/// Return every bare (non `[key]=value`) element of an associative array
/// compound assignment so the caller can emit the GNU "must use subscript"
/// error for each one. Returns an empty vec for the alternating `key value`
/// form (no explicit subscripts) — that form has no bare elements.
pub(in crate::executor) fn assoc_bare_elements(value: &str) -> Vec<String> {
    let tokens = array_assignment_tokens(value);
    let explicit_subscripts = tokens
        .iter()
        .any(|token| assoc_assignment_token(token).is_some());
    if !explicit_subscripts {
        return Vec::new();
    }
    tokens
        .iter()
        .filter(|token| assoc_assignment_token(token).is_none())
        .map(|token| unquote_storage_value(token))
        .collect()
}

fn assoc_assignment_token(token: &str) -> Option<(&str, &str, bool)> {
    if let Some((left, rhs)) = token.split_once("+=") {
        if let Some(key) = left
            .strip_prefix('[')
            .and_then(|left| left.strip_suffix(']'))
        {
            return Some((key, rhs, true));
        }
    }

    let (left, rhs) = token.split_once('=')?;
    let key = left
        .strip_prefix('[')
        .and_then(|left| left.strip_suffix(']'))?;
    Some((key, rhs, false))
}

pub(in crate::executor) fn append_assoc_scalar_value(current: &str, value: &str) -> String {
    let mut entries = assoc_entries(current);
    let value = unquote_storage_value(value);
    if let Some((_, entry_value)) = entries.iter_mut().rev().find(|(key, _)| key == "0") {
        *entry_value = value;
    } else {
        entries.push(("0".to_string(), value));
    }
    format_assoc_storage(entries)
}

pub(in crate::executor) fn format_assoc_storage(entries: Vec<(String, String)>) -> String {
    format!(
        "({})",
        entries
            .into_iter()
            .map(|(key, value)| {
                format!(
                    "[{}]={}",
                    quote_assoc_key(&key),
                    quote_assoc_storage_value(&value)
                )
            })
            .collect::<Vec<_>>()
            .join(" ")
    )
}

pub(in crate::executor) fn quote_assoc_key(key: &str) -> String {
    if !key.is_empty()
        && !key
            .chars()
            .any(|ch| ch.is_ascii_whitespace() || matches!(ch, '"' | '\\' | ']'))
    {
        return key.to_string();
    }

    quote_assoc_storage_value_forced(key)
}

pub(in crate::executor) fn quote_assoc_storage_value(value: &str) -> String {
    if !value.is_empty()
        && !value
            .chars()
            .any(|ch| ch.is_ascii_whitespace() || matches!(ch, '"' | '\\'))
    {
        return value.to_string();
    }

    let mut quoted = String::from("\"");
    for ch in value.chars() {
        if matches!(ch, '"' | '\\') {
            quoted.push('\\');
        }
        quoted.push(ch);
    }
    quoted.push('"');
    quoted
}

fn quote_assoc_storage_value_forced(value: &str) -> String {
    let mut quoted = String::from("\"");
    for ch in value.chars() {
        if matches!(ch, '"' | '\\') {
            quoted.push('\\');
        }
        quoted.push(ch);
    }
    quoted.push('"');
    quoted
}

pub(in crate::executor) fn assoc_entries(value: &str) -> Vec<(String, String)> {
    let Some(inner) = value
        .strip_prefix('(')
        .and_then(|value| value.strip_suffix(')'))
    else {
        return Vec::new();
    };

    split_storage_words(inner)
        .filter_map(|part| {
            let (key, value) = part.split_once('=')?;
            Some((
                unquote_storage_value(key.trim_start_matches('[').trim_end_matches(']')),
                unquote_storage_value(value),
            ))
        })
        .collect()
}

pub(in crate::executor) fn assoc_value_at(value: &str, key: &str) -> Option<String> {
    assoc_entries(value)
        .into_iter()
        .rev()
        .find_map(|(entry_key, entry_value)| (entry_key == key).then_some(entry_value))
}

pub(in crate::executor) fn assoc_keys(value: &str) -> Vec<String> {
    bash_assoc_order(&assoc_entries(value))
        .into_iter()
        .map(|(_, (_, key))| key)
        .collect()
}

/// FNV-1 (multiply first, then xor) over `char` bytes, 32 bit — hashlib.c
/// hash_string. On x86 a plain `char` is signed, so bytes >= 0x80 sign-
/// extend before the xor.
fn bash_hash_string(key: &str) -> u32 {
    let mut hash: u32 = 2166136261;
    for byte in key.bytes() {
        hash = hash.wrapping_mul(16777619);
        hash ^= (byte as i8) as i32 as u32;
    }
    hash
}

/// Bash assoc.c / hashlib.c table iteration order: FNV-1 hashed keys into a
/// power-of-two bucket array starting at 128 buckets, head-insertion chains,
/// grow x4 when nentries >= nbuckets * 2 (rehash walks old buckets 0..n and
/// re-inserts each item at its new chain head). Iteration visits bucket 0..n,
/// each chain head to tail. A repeated key keeps its first-insert slot and
/// the last value wins (hash_search replaces data in place).
pub(crate) fn bash_assoc_order(
    entries: &[(String, String)],
) -> Vec<(usize, (String, String))> {
    // First occurrence fixes the slot; last occurrence supplies the value.
    let mut first_index: std::collections::HashMap<&str, usize> =
        std::collections::HashMap::new();
    let mut unique: Vec<usize> = Vec::new();
    for (index, (key, _)) in entries.iter().enumerate() {
        if first_index.contains_key(key.as_str()) {
            continue;
        }
        first_index.insert(key.as_str(), index);
        unique.push(index);
    }

    let mut nbuckets: usize = 128;
    let mut buckets: Vec<Vec<usize>> = vec![Vec::new(); nbuckets];
    let mut count = 0usize;
    for &entry_index in &unique {
        if count >= nbuckets * 2 {
            let mut grown = vec![Vec::new(); nbuckets * 4];
            for old_bucket in &buckets {
                for &item in old_bucket {
                    let bucket = bash_hash_string(&entries[item].0) as usize & (nbuckets * 4 - 1);
                    grown[bucket].insert(0, item);
                }
            }
            buckets = grown;
            nbuckets *= 4;
        }
        let bucket = bash_hash_string(&entries[entry_index].0) as usize & (nbuckets - 1);
        buckets[bucket].insert(0, entry_index);
        count += 1;
    }

    buckets
        .into_iter()
        .flatten()
        .map(|index| (index, entries[index].clone()))
        .collect()
}

pub(in crate::executor) fn assoc_hash_ordered_entries(value: &str) -> Vec<(String, String)> {
    bash_assoc_order(&assoc_entries(value))
        .into_iter()
        .map(|(_, entry)| entry)
        .collect()
}

pub(in crate::executor) fn assoc_hash_ordered_values(value: &str) -> Vec<String> {
    bash_assoc_order(&assoc_entries(value))
        .into_iter()
        .map(|(_, (_, entry_value))| entry_value)
        .collect()
}

pub(in crate::executor) fn split_storage_words(value: &str) -> impl Iterator<Item = String> + '_ {
    StorageWordIter {
        input: value,
        offset: 0,
    }
}

struct StorageWordIter<'a> {
    input: &'a str,
    offset: usize,
}

impl Iterator for StorageWordIter<'_> {
    type Item = String;

    fn next(&mut self) -> Option<Self::Item> {
        while let Some(ch) = self.input.get(self.offset..)?.chars().next() {
            if !ch.is_ascii_whitespace() {
                break;
            }
            self.offset += ch.len_utf8();
        }

        let mut word = String::new();
        let mut in_double = false;
        let mut escaped = false;
        for (relative, ch) in self.input[self.offset..].char_indices() {
            if escaped {
                word.push(ch);
                escaped = false;
                continue;
            }
            if ch == '\\' && in_double {
                word.push(ch);
                escaped = true;
                continue;
            }
            if ch == '"' {
                in_double = !in_double;
                word.push(ch);
                continue;
            }
            if ch.is_ascii_whitespace() && !in_double {
                self.offset += relative + ch.len_utf8();
                return Some(word);
            }
            word.push(ch);
        }
        self.offset = self.input.len();
        (!word.is_empty()).then_some(word)
    }
}

pub(in crate::executor) fn unquote_storage_value(value: &str) -> String {
    fn restore_quote_markers(value: &str) -> String {
        value
            .replace('\x1f', "$")
            .replace('\x1a', "`")
            .replace('\x17', "'")
            .replace('\x14', "\\")
    }

    if value == "\\\"\\" {
        return "\"\"".to_string();
    }

    if let Some(inner) = value
        .strip_prefix('\'')
        .and_then(|value| value.strip_suffix('\''))
    {
        return restore_quote_markers(inner);
    }

    let Some(inner) = value
        .strip_prefix('"')
        .and_then(|value| value.strip_suffix('"'))
    else {
        return restore_quote_markers(value);
    };

    let mut unquoted = String::new();
    let mut escaped = false;
    for ch in inner.chars() {
        if escaped {
            if !matches!(ch, '$' | '`' | '"' | '\\' | '\n') {
                unquoted.push('\\');
            }
            unquoted.push(ch);
            escaped = false;
        } else if ch == '\\' {
            escaped = true;
        } else {
            unquoted.push(ch);
        }
    }
    if escaped {
        unquoted.push('\\');
    }
    if unquoted == "\\\"\\" {
        return "\"\"".to_string();
    }
    restore_quote_markers(&unquoted)
}

pub(in crate::executor) fn quote_compound_field_value(value: &str) -> String {
    quote_array_value(value)
}
