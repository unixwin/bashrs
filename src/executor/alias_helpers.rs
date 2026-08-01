pub(in crate::executor) fn split_shell_words(source: &str) -> Vec<String> {
    let mut words = Vec::new();
    let mut current = String::new();
    let mut quote = None;
    for ch in source.chars() {
        match (ch, quote) {
            ('\'' | '"', None) => quote = Some(ch),
            (q, Some(active)) if q == active => quote = None,
            (' ' | '\t', None) => {
                if !current.is_empty() {
                    words.push(std::mem::take(&mut current));
                }
            }
            _ => current.push(ch),
        }
    }
    if !current.is_empty() {
        words.push(current);
    }
    words
}

pub(in crate::executor) fn split_first_shell_word(source: &str) -> Option<(String, &str)> {
    let trimmed = source.trim_start();
    let offset = source.len() - trimmed.len();
    let mut quote = None;
    for (index, ch) in trimmed.char_indices() {
        match (ch, quote) {
            ('\'' | '"', None) => quote = Some(ch),
            (q, Some(active)) if q == active => quote = None,
            (' ' | '\t' | '\n' | '\r', None) => {
                let word = trimmed[..index].to_string();
                let remainder = &source[offset + index + ch.len_utf8()..];
                return Some((word, remainder));
            }
            _ => {}
        }
    }

    if trimmed.is_empty() {
        None
    } else {
        Some((trimmed.to_string(), ""))
    }
}

pub(in crate::executor) fn split_unquoted_and_and(source: &str) -> Option<(&str, &str)> {
    let mut single = false;
    let mut double = false;
    let mut escaped = false;
    let chars = source.char_indices().collect::<Vec<_>>();
    let mut index = 0;

    while index < chars.len() {
        let (byte_index, ch) = chars[index];
        if escaped {
            escaped = false;
            index += 1;
            continue;
        }
        if ch == '\\' && !single {
            escaped = true;
            index += 1;
            continue;
        }
        match ch {
            '\'' if !double => single = !single,
            '"' if !single => double = !double,
            '&' if !single && !double && chars.get(index + 1).is_some_and(|(_, ch)| *ch == '&') => {
                return Some((&source[..byte_index], &source[byte_index + 2..]));
            }
            _ => {}
        }
        index += 1;
    }

    None
}
