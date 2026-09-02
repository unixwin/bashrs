use super::*;

pub(in crate::executor) fn read_array_storage(values: &[String]) -> String {
    let rendered = values
        .iter()
        .enumerate()
        .map(|(index, value)| format!("[{index}]={}", render_read_array_element(value)))
        .collect::<Vec<_>>()
        .join(" ");
    format!("\x1d({rendered})")
}
pub(in crate::executor) fn render_read_array_element(value: &str) -> String {
    if value.contains(['\n', '\r']) {
        let mut rendered = String::from("$'");
        for ch in value.chars() {
            match ch {
                '\n' => rendered.push_str("\\n"),
                '\r' => rendered.push_str("\\r"),
                '\\' => rendered.push_str("\\\\"),
                '\'' => rendered.push_str("\\'"),
                other => rendered.push(other),
            }
        }
        rendered.push('\'');
        return rendered;
    }

    format!("\"{}\"", value.replace('\\', "\\\\").replace('"', "\\\""))
}

fn ifs_whitespace(ch: char, ifs: &str) -> bool {
    ch.is_whitespace() && ifs.contains(ch)
}

/// Split a line into read fields using IFS, returning the byte ranges of each
/// field in the original line. Bash keeps trailing empty fields produced by a
/// trailing IFS delimiter (unlike word-splitting for expansion), so every
/// delimiter boundary becomes a field. Only a leading run of IFS *whitespace*
/// is elided (it does not create a leading empty field).
///
/// Ranges are byte offsets into `line` and always land on UTF-8 character
/// boundaries, so callers can slice `line` directly without panicking on
/// multi-byte characters (GNU read splits at multibyte character boundaries,
/// mirroring lib/sh/stringlib.c / subst.c).
fn split_read_field_ranges(line: &str, ifs: &str) -> Vec<(usize, usize)> {
    // Track each char's byte offset (char_indices) so the returned ranges
    // slice `line` at UTF-8 boundaries instead of splitting multi-byte
    // sequences mid-character.
    let chars: Vec<(usize, char)> = line.char_indices().collect();
    let n = chars.len();
    let line_len = line.len();
    if ifs.is_empty() {
        return vec![(0, line_len)];
    }

    let mut ranges: Vec<(usize, usize)> = Vec::new();
    let mut i = 0usize;
    let mut ended_at_delimiter = false;
    // Skip a leading run of IFS whitespace (does not create a leading field).
    while i < n && ifs_whitespace(chars[i].1, ifs) {
        i += 1;
    }
    while i < n {
        let start = chars[i].0;
        // A field runs until the next IFS character. IFS whitespace inside a
        // field is kept literally; a non-whitespace IFS delimiter ends it.
        while i < n && !ifs.contains(chars[i].1) {
            i += 1;
        }
        let end = if i < n { chars[i].0 } else { line_len };
        ranges.push((start, end));
        if i >= n {
            ended_at_delimiter = false;
            break;
        }
        // Consume the delimiter and any following IFS whitespace. A non-whitespace
        // IFS delimiter ends exactly one field; trailing IFS whitespace is skipped so
        // it does not spawn an extra empty field.
        if ifs_whitespace(chars[i].1, ifs) {
            ended_at_delimiter = true;
            while i < n && ifs_whitespace(chars[i].1, ifs) {
                i += 1;
            }
            // An IFS whitespace run adjacent to a non-whitespace delimiter forms
            // one delimiter sequence; do not expose the latter as an empty field.
            if i < n && ifs.contains(chars[i].1) && !ifs_whitespace(chars[i].1, ifs) {
                i += 1;
                while i < n && ifs_whitespace(chars[i].1, ifs) {
                    i += 1;
                }
            }
        } else {
            ended_at_delimiter = true;
            i += 1;
            while i < n && ifs_whitespace(chars[i].1, ifs) {
                i += 1;
            }
        }
    }
    // A final IFS delimiter creates a trailing empty field which bash drops
    // (mirroring word-splitting's omission of a trailing empty field).
    if ended_at_delimiter {
        ranges.push((line_len, line_len));
        if let Some((start, end)) = ranges.last().copied() {
            if start == end {
                ranges.pop();
            }
        }
    }
    ranges
}
fn field_value(
    line: &str,
    range: (usize, usize),
    ifs: &str,
    interpret_backslashes: bool,
) -> String {
    let raw: String = line[range.0..range.1].chars().collect();
    let value = if interpret_backslashes {
        unescape_read_backslashes(&raw)
    } else {
        raw
    };
    // Trailing IFS whitespace is trimmed from each assigned field value.
    value
        .trim_end_matches(|ch: char| ifs_whitespace(ch, ifs))
        .to_string()
}

pub(in crate::executor) fn read_scalar_fields(
    line: &str,
    names_len: usize,
    ifs: &str,
) -> Vec<String> {
    read_scalar_fields_internal(line, names_len, ifs, false)
}

pub(in crate::executor) fn read_scalar_fields_with_backslashes(
    line: &str,
    names_len: usize,
    ifs: &str,
) -> Vec<String> {
    read_scalar_fields_internal(line, names_len, ifs, true)
}

fn read_scalar_fields_internal(
    line: &str,
    names_len: usize,
    ifs: &str,
    interpret_backslashes: bool,
) -> Vec<String> {
    if names_len == 0 {
        return Vec::new();
    }
    if names_len == 1 {
        let value = if interpret_backslashes {
            unescape_read_backslashes(line)
        } else {
            line.to_string()
        };
        // A single name receives the whole (leading/trailing IFS-trimmed) line.
        return vec![value
            .trim_matches(|ch: char| ifs_whitespace(ch, ifs))
            .to_string()];
    }
    if ifs.is_empty() {
        let mut fields = vec![if interpret_backslashes {
            unescape_read_backslashes(line)
        } else {
            line.to_string()
        }];
        while fields.len() < names_len {
            fields.push(String::new());
        }
        return fields;
    }

    let ranges = split_read_field_ranges(line, ifs);
    let field_count = ranges.len();

    // Assign the first names_len-1 names to the first fields directly.
    let mut fields: Vec<String> = Vec::with_capacity(names_len);
    for index in 0..names_len.saturating_sub(1) {
        let value = ranges
            .get(index)
            .map(|range| field_value(line, *range, ifs, interpret_backslashes))
            .unwrap_or_default();
        fields.push(value);
    }

    // The last name receives the remainder. When there are at least as many
    // fields as names, it gets the final field value. When there are *more*
    // fields than names, bash joins the remaining fields with their delimiters
    // by assigning the raw line tail from the start of the last named field.
    let last_value = if field_count >= names_len {
        let range = ranges[names_len - 1];
        if field_count == names_len {
            field_value(line, range, ifs, interpret_backslashes)
        } else {
            let tail: String = line[range.0..].chars().collect();
            let tail = if interpret_backslashes {
                unescape_read_backslashes(&tail)
            } else {
                tail
            };
            tail.trim_end_matches(|ch: char| ifs_whitespace(ch, ifs))
                .to_string()
        }
    } else {
        String::new()
    };
    fields.push(last_value);

    while fields.len() < names_len {
        fields.push(String::new());
    }
    fields
}

pub(crate) fn mark_env_name(env_vars: &mut HashMap<String, String>, key: &str, name: &str) {
    let mut names: Vec<String> = env_vars
        .get(key)
        .map(|value| {
            value
                .split('\x1f')
                .filter(|name| !name.is_empty())
                .map(str::to_string)
                .collect()
        })
        .unwrap_or_default();
    if !names.iter().any(|current| current == name) {
        names.push(name.to_string());
    }
    env_vars.insert(key.to_string(), names.join("\x1f"));
}
