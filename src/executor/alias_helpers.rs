pub(in crate::executor) fn split_shell_words(source: &str) -> Vec<String> {
    let mut words = Vec::new();
    let mut current = String::new();
    let mut quote = None;
    let mut backtick = false;
    let mut chars = source.chars().peekable();
    while let Some(ch) = chars.next() {
        if backtick {
            current.push(ch);
            if ch == '\\' {
                if let Some(escaped) = chars.next() {
                    current.push(escaped);
                }
            } else if ch == '`' {
                backtick = false;
            }
            continue;
        }

        match (ch, quote) {
            ('$', None) if chars.peek().copied() == Some('(') => {
                copy_dollar_paren_word(&mut current, &mut chars);
            }
            ('`', None) => {
                backtick = true;
                current.push(ch);
            }
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

fn copy_dollar_paren_word(
    current: &mut String,
    chars: &mut std::iter::Peekable<std::str::Chars<'_>>,
) {
    current.push('$');
    if chars.next() != Some('(') {
        return;
    }
    current.push('(');

    let mut depth = 1usize;
    while let Some(ch) = chars.next() {
        current.push(ch);
        match ch {
            '\\' => {
                if let Some(escaped) = chars.next() {
                    current.push(escaped);
                }
            }
            '\'' => copy_quoted_word_part(current, chars, '\''),
            '"' => copy_quoted_word_part(current, chars, '"'),
            '`' => copy_backtick_word_part(current, chars),
            '$' if chars.peek().copied() == Some('(') => {
                chars.next();
                current.push('(');
                depth += 1;
            }
            '(' => depth += 1,
            ')' => {
                depth = depth.saturating_sub(1);
                if depth == 0 {
                    break;
                }
            }
            _ => {}
        }
    }
}

fn copy_quoted_word_part(
    current: &mut String,
    chars: &mut std::iter::Peekable<std::str::Chars<'_>>,
    quote: char,
) {
    while let Some(ch) = chars.next() {
        current.push(ch);
        if ch == '\\' && quote != '\'' {
            if let Some(escaped) = chars.next() {
                current.push(escaped);
            }
        } else if ch == quote {
            break;
        }
    }
}

fn copy_backtick_word_part(
    current: &mut String,
    chars: &mut std::iter::Peekable<std::str::Chars<'_>>,
) {
    while let Some(ch) = chars.next() {
        current.push(ch);
        if ch == '\\' {
            if let Some(escaped) = chars.next() {
                current.push(escaped);
            }
        } else if ch == '`' {
            break;
        }
    }
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
    split_unquoted_token(source, "&&")
}

pub(in crate::executor) fn split_unquoted_semicolon(source: &str) -> Option<(&str, &str)> {
    split_unquoted_token(source, ";")
}

fn split_unquoted_token<'a>(source: &'a str, token: &str) -> Option<(&'a str, &'a str)> {
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
            _ if !single && !double && source[byte_index..].starts_with(token) => {
                return Some((&source[..byte_index], &source[byte_index + token.len()..]));
            }
            _ => {}
        }
        index += 1;
    }

    None
}
