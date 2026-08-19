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

fn trim_single_scalar_field(value: &str, ifs: &str) -> String {
    if ifs.is_empty() {
        return value.to_string();
    }

    value
        .trim_matches(|ch: char| ch.is_whitespace() && ifs.contains(ch))
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
        return vec![trim_single_scalar_field(&value, ifs)];
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

    let chars = line.chars().collect::<Vec<_>>();
    let mut fields = Vec::new();
    let mut index = 0usize;
    while fields.len() + 1 < names_len {
        skip_ifs_whitespace(&chars, &mut index, ifs);
        if index >= chars.len() {
            fields.push(String::new());
            continue;
        }

        let mut current = String::new();
        while index < chars.len() {
            let ch = chars[index];
            if interpret_backslashes && ch == '\\' {
                index += 1;
                match chars.get(index).copied() {
                    Some('\n') => index += 1,
                    Some('\r') if chars.get(index + 1) == Some(&'\n') => index += 2,
                    Some(next) => {
                        current.push(next);
                        index += 1;
                    }
                    None => {}
                }
                continue;
            }

            if ifs.contains(ch) {
                consume_read_delimiter(&chars, &mut index, ifs, ch);
                break;
            }

            current.push(ch);
            index += 1;
        }
        fields.push(current);
    }

    skip_ifs_whitespace(&chars, &mut index, ifs);
    let rest = chars[index..].iter().collect::<String>();
    let rest = if interpret_backslashes {
        unescape_read_backslashes(&rest)
    } else {
        rest
    };
    fields.push(trim_single_scalar_field(&rest, ifs));
    while fields.len() < names_len {
        fields.push(String::new());
    }
    fields
}

fn ifs_whitespace(ch: char, ifs: &str) -> bool {
    ch.is_whitespace() && ifs.contains(ch)
}

fn skip_ifs_whitespace(chars: &[char], index: &mut usize, ifs: &str) {
    while chars.get(*index).is_some_and(|ch| ifs_whitespace(*ch, ifs)) {
        *index += 1;
    }
}

fn consume_read_delimiter(chars: &[char], index: &mut usize, ifs: &str, delimiter: char) {
    *index += 1;
    if ifs_whitespace(delimiter, ifs) {
        skip_ifs_whitespace(chars, index, ifs);
        if chars
            .get(*index)
            .is_some_and(|ch| ifs.contains(*ch) && !ifs_whitespace(*ch, ifs))
        {
            *index += 1;
            skip_ifs_whitespace(chars, index, ifs);
        }
    } else {
        skip_ifs_whitespace(chars, index, ifs);
    }
}

pub(super) fn mark_env_name(env_vars: &mut HashMap<String, String>, key: &str, name: &str) {
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
