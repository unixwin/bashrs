pub(super) fn is_keyword(word: &str) -> bool {
    matches!(
        word,
        "if" | "then"
            | "else"
            | "elif"
            | "fi"
            | "while"
            | "do"
            | "done"
            | "until"
            | "for"
            | "case"
            | "esac"
            | "in"
            | "function"
            | "select"
            | "time"
            | "coproc"
    )
}

pub(super) fn is_assignment(word: &str) -> bool {
    let Some(pos) = word.find('=') else {
        return false;
    };
    let var_name = word[..pos].strip_suffix('+').unwrap_or(&word[..pos]);
    !var_name.is_empty()
        && var_name
            .chars()
            .next()
            .is_some_and(|c| c.is_ascii_alphabetic() || c == '_')
        && var_name
            .chars()
            .all(|c| c.is_ascii_alphanumeric() || c == '_')
}


pub(super) fn is_brace_expansion(word: &str) -> bool {
    word.starts_with('{')
        && word.ends_with('}')
        && word.len() >= 3
        && !word.chars().any(char::is_whitespace)
        && (word[1..word.len() - 1].contains("..") || word.contains(','))
}

pub(super) fn is_word_delimiter(ch: char) -> bool {
    " \t\n|&;<>(){}".contains(ch)
}

pub(super) fn assignment_value_is_quoted(raw: &str) -> bool {
    let Some((_, value)) = raw.split_once('=') else {
        return false;
    };

    // Quotes inside a `${...}` body belong to the expansion itself (GNU keeps
    // them for the expansion stage), not to the assignment's quoting state.
    let mut in_backtick = false;
    let mut escaped = false;
    let mut expansion_depth = 0usize;
    let mut in_single = false;
    let mut in_double = false;
    let mut chars = value.chars().peekable();
    while let Some(ch) = chars.next() {
        if escaped {
            escaped = false;
            continue;
        }

        if ch == '\\' && !in_single {
            escaped = true;
            continue;
        }

        if ch == '`' && !in_single {
            in_backtick = !in_backtick;
            continue;
        }

        if expansion_depth > 0 {
            match ch {
                '\'' if !in_double => in_single = !in_single,
                '"' if !in_single => in_double = !in_double,
                '$' if !in_single && !in_double && chars.peek() == Some(&'{') => {
                    chars.next();
                    expansion_depth += 1;
                }
                '}' if !in_single && !in_double => {
                    expansion_depth -= 1;
                    if expansion_depth == 0 {
                        in_single = false;
                        in_double = false;
                    }
                }
                _ => {}
            }
            continue;
        }

        if ch == '$' && chars.peek() == Some(&'{') {
            chars.next();
            expansion_depth = 1;
            in_single = false;
            in_double = false;
            continue;
        }

        if !in_backtick && matches!(ch, '"' | '\'') {
            return true;
        }
    }

    false
}

pub(super) fn mark_quoted_assignment_value(raw: &str, value: &str) -> String {
    let Some((name, rhs)) = value.split_once('=') else {
        return value.to_string();
    };
    let raw_rhs = raw.split_once('=').map(|(_, rhs)| rhs).unwrap_or_default();
    let rhs = if raw_rhs.starts_with('\"')
        && raw_rhs.ends_with('\"')
        && !raw_rhs.contains("$(")
        && !raw_rhs.contains('`')
    {
        rhs.replace('\'', "\x16")
    } else {
        rhs.to_string()
    };

    format!("{name}=\x1c{rhs}")
}

pub(super) fn quoted_literal_tilde(raw: &str, value: &str) -> bool {
    value.starts_with('~')
        && ((raw.starts_with('\'') && raw.ends_with('\''))
            || (raw.starts_with('"') && raw.ends_with('"')))
}
