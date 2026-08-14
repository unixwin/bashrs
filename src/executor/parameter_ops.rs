use super::*;

pub(in crate::executor) fn decode_parameter_word_quotes(word: &str) -> String {
    let mut output = String::new();
    let chars = word.chars().collect::<Vec<_>>();
    let mut index = 0;
    while index < chars.len() {
        match chars[index] {
            '\x17' => {
                output.push('\'');
                index += 1;
            }
            '"' => {
                index += 1;
                while index < chars.len() {
                    let ch = chars[index];
                    index += 1;
                    if ch == '"' {
                        break;
                    }
                    output.push(ch);
                }
            }
            '\'' => {
                if let Some(close_offset) = chars[index + 1..].iter().position(|ch| *ch == '\'') {
                    let close = index + 1 + close_offset;
                    for ch in &chars[index + 1..close] {
                        output.push(*ch);
                    }
                    index = close + 1;
                } else {
                    output.push('\'');
                    index += 1;
                }
            }
            ch => {
                output.push(ch);
                index += 1;
            }
        }
    }
    output
}

pub(in crate::executor) fn decode_parameter_replacement_quotes(replacement: &str) -> String {
    const PROTECTED_BACKSLASH_QUOTE: char = '\x16';
    const PROTECTED_LITERAL_BACKSLASH: char = '\x19';
    // The lexer preserves an escaped backslash as \x14 so replacement
    // decoding can distinguish `\\n` (literal `\\n`) from `\n` (the
    // backslash is consumed by Bash's replacement parser).
    let replacement = replacement.replace('\x14', &PROTECTED_LITERAL_BACKSLASH.to_string());
    // In a parameter replacement, backslashes are data (and `\&` is
    // interpreted later by replace_with_amp).  The pattern decoder cannot be
    // reused here because it intentionally removes a backslash while
    // decoding a quoted pattern character such as `\}`.
    if replacement.contains('"') {
        // A replacement may contain a separately quoted pattern such as
        // `"'\\''"`.  Those double quotes delimit shell syntax; they must
        // not become part of the replacement while the backslashes inside
        // remain literal replacement data.
        let mut output = String::new();
        let mut chars = replacement.chars().peekable();
        while let Some(ch) = chars.next() {
            if ch == '"' {
                continue;
            }
            if ch == '\\' && chars.peek() == Some(&'\x17') {
                output.push('\\');
                output.push('\'');
                chars.next();
                continue;
            }
            if ch == '\\' && chars.peek() == Some(&'\\') {
                output.push('\\');
                chars.next();
                continue;
            }
            output.push(if ch == '\x17' { '\'' } else { ch });
        }
        return output.replace(PROTECTED_LITERAL_BACKSLASH, "\\");
    }

    if replacement.contains('\x17') || replacement.contains("\\'") {
        // Quote removal has already encoded escaped single quotes as \x17 in
        // some lexer paths.  In the remaining paths an escaped quote arrives
        // as `\\'`; both forms denote a literal quote in the replacement.
        let mut output = String::new();
        let mut chars = replacement.chars().peekable();
        while let Some(ch) = chars.next() {
            if ch == '\x17' {
                output.push('\'');
            } else if ch == '\\' && chars.peek() == Some(&'\'') {
                output.push('\'');
                chars.next();
            } else if ch == '\\' && chars.peek() == Some(&'\\') {
                output.push('\\');
                output.push('\\');
                chars.next();
                if chars.peek() == Some(&'\'') {
                    chars.next();
                }
                if chars.peek() == Some(&'\\') {
                    chars.next();
                    if chars.peek() == Some(&'\'') {
                        chars.next();
                    }
                }
            } else {
                output.push(ch);
            }
        }
        return output.replace(PROTECTED_LITERAL_BACKSLASH, "\\");
    }

    if replacement.contains('\\') {
        let mut output = String::new();
        let chars = replacement.chars().collect::<Vec<_>>();
        let mut index = 0;
        while index < chars.len() {
            if chars[index] == '\\' && chars.get(index + 1) == Some(&'\x17') {
                output.push(PROTECTED_BACKSLASH_QUOTE);
                index += 2;
            } else if chars[index] == '\\' && chars.get(index + 1) == Some(&'\x18') {
                output.push('\\');
                index += 2;
            } else {
                output.push(chars[index]);
                index += 1;
            }
        }
        return normalize_parameter_replacement_escapes(&output.replace('\x18', "\\"))
            .replace(PROTECTED_LITERAL_BACKSLASH, "\\");
    }
    let mut protected = String::new();
    let chars = replacement.chars().collect::<Vec<_>>();
    let mut index = 0;
    while index < chars.len() {
        if chars[index] == '\\' && chars.get(index + 1) == Some(&'\x17') {
            protected.push(PROTECTED_BACKSLASH_QUOTE);
            index += 2;
            continue;
        }
        protected.push(chars[index]);
        index += 1;
    }
    normalize_parameter_replacement_escapes(
        &decode_parameter_pattern_quotes(&protected).replace('\x18', "\\"),
    )
    .replace(PROTECTED_LITERAL_BACKSLASH, "\\")
}

/// Remove the quoting backslash that Bash consumes while parsing a
/// parameter-substitution replacement.  `\&` is retained for the later
/// replacement pass, where it means a literal ampersand; a doubled
/// backslash becomes one literal backslash.
fn normalize_parameter_replacement_escapes(value: &str) -> String {
    let mut output = String::with_capacity(value.len());
    let mut chars = value.chars().peekable();
    while let Some(ch) = chars.next() {
        if ch != '\\' {
            output.push(ch);
            continue;
        }

        match chars.peek().copied() {
            Some('&') => output.push('\\'),
            Some('\'') => output.push('\\'),
            Some('\\') => {
                chars.next();
                output.push('\\');
            }
            Some(next) => {
                chars.next();
                output.push(next);
            }
            None => output.push('\\'),
        }
    }
    output
}

pub(in crate::executor) fn restore_protected_replacement_quotes(value: &str) -> String {
    value.replace('\x16', "\\'")
}

pub(in crate::executor) fn parse_parameter_error_operator(
    inner: &str,
) -> Option<(&str, &str, bool)> {
    if let Some((name, message)) = inner.split_once(":?") {
        if is_parameter_error_name(name) {
            return Some((name, message, true));
        }
    }

    if let Some((name, message)) = inner.split_once('?') {
        if is_parameter_error_name(name) {
            return Some((name, message, false));
        }
    }

    None
}

pub(in crate::executor) fn parse_parameter_assignment_operator(
    inner: &str,
) -> Option<(&str, bool)> {
    if let Some((name, _)) = inner.split_once(":=") {
        if is_shell_name(name)
            || name.parse::<usize>().is_ok_and(|index| index > 0)
            || parse_array_subscript(name).is_some()
        {
            return Some((name, true));
        }
    }

    if let Some((name, _)) = inner.split_once('=') {
        if is_shell_name(name)
            || name.parse::<usize>().is_ok_and(|index| index > 0)
            || parse_array_subscript(name).is_some()
        {
            return Some((name, false));
        }
    }

    None
}

pub(in crate::executor) fn matching_parameter_brace(input: &str) -> Option<usize> {
    let mut chars = input.char_indices().peekable();
    let mut depth = 0usize;
    let mut in_bracket_expression = false;
    while let Some((index, ch)) = chars.next() {
        // GNU Bash treats `\` plus the next character as one unit while
        // scanning `${...}` (extract_dollar_brace_string advances by two).
        // `\\` is a literal backslash, so the following `}` still closes.
        if ch == '\\' {
            chars.next();
            continue;
        }
        if ch == '[' {
            in_bracket_expression = true;
            continue;
        }
        if ch == ']' && in_bracket_expression {
            in_bracket_expression = false;
            continue;
        }
        if ch == '$' && chars.peek().is_some_and(|(_, ch)| *ch == '{') {
            chars.next();
            depth += 1;
            continue;
        }
        if ch == '}' && !in_bracket_expression {
            if depth == 0 {
                return Some(index);
            }
            depth -= 1;
        }
    }
    None
}

pub(in crate::executor) fn braced_parameter_spans_whole_word(word: &str) -> bool {
    let Some(rest) = word.strip_prefix("${") else {
        return false;
    };
    matching_parameter_brace(rest).is_some_and(|index| index + 1 == rest.len())
}

pub(in crate::executor) fn command_substitution_spans_whole_word(word: &str) -> bool {
    let Some(rest) = word.strip_prefix("$(") else {
        return false;
    };

    let mut depth = 1usize;
    let mut single = false;
    let mut double = false;
    let mut escaped = false;
    for (index, ch) in rest.char_indices() {
        if escaped {
            escaped = false;
            continue;
        }
        if ch == '\\' && !single {
            escaped = true;
            continue;
        }
        match ch {
            '\'' if !double => single = !single,
            '"' if !single => double = !double,
            '(' if !single && !double => depth += 1,
            ')' if !single && !double => {
                depth = depth.saturating_sub(1);
                if depth == 0 {
                    return index + ch.len_utf8() == rest.len();
                }
            }
            _ => {}
        }
    }
    false
}

pub(in crate::executor) fn backtick_substitution_spans_whole_word(word: &str) -> bool {
    let Some(rest) = word.strip_prefix('`') else {
        return false;
    };

    let mut escaped = false;
    for (index, ch) in rest.char_indices() {
        if escaped {
            escaped = false;
            continue;
        }
        if ch == '\\' {
            escaped = true;
            continue;
        }
        if ch == '`' {
            return index + ch.len_utf8() == rest.len();
        }
    }
    false
}

pub(in crate::executor) fn is_parameter_error_name(name: &str) -> bool {
    is_shell_name(name)
        || name
            .strip_prefix('!')
            .is_some_and(|name| !name.is_empty() && is_shell_name(name))
        || matches!(name, "#" | "@" | "*" | "?" | "$" | "-" | "0")
        || name.parse::<usize>().is_ok()
        || parse_array_subscript(name).is_some()
}

pub(in crate::executor) fn has_indirect_parameter_word_operator(name: &str) -> bool {
    let Some(indirect) = name.strip_prefix('!') else {
        return false;
    };
    [":-", ":=", ":?", ":+", "-", "=", "?", "+"]
        .iter()
        .any(|operator| {
            indirect
                .split_once(operator)
                .is_some_and(|(left, _)| !left.is_empty())
        })
}

pub(in crate::executor) fn parameter_substring(
    value: &str,
    offset: isize,
    length: Option<isize>,
) -> String {
    let char_count = value.chars().count();
    let Some(start) = parameter_substring_start(char_count, offset) else {
        return String::new();
    };
    let take = match length {
        Some(length) if length < 0 => {
            let remaining = char_count.saturating_sub(start);
            remaining.saturating_sub(length.unsigned_abs())
        }
        Some(length) => usize::try_from(length).unwrap_or(usize::MAX),
        None => usize::MAX,
    };

    value.chars().skip(start).take(take).collect()
}

pub(in crate::executor) fn parameter_substring_start(
    char_count: usize,
    offset: isize,
) -> Option<usize> {
    if offset < 0 {
        char_count.checked_sub(offset.unsigned_abs())
    } else {
        usize::try_from(offset)
            .ok()
            .filter(|start| *start <= char_count)
    }
}

pub(in crate::executor) fn parameter_substring_has_negative_result(
    char_count: usize,
    offset: isize,
    length: isize,
) -> bool {
    if length >= 0 {
        return false;
    }
    let Some(start) = parameter_substring_start(char_count, offset) else {
        return false;
    };
    (char_count as i128) - (start as i128) + (length as i128) < 0
}

pub(in crate::executor) fn positional_parameter_substring(
    params: &[String],
    offset: isize,
    length: Option<isize>,
) -> Vec<String> {
    let start = if offset < 0 {
        params
            .len()
            .checked_sub(offset.unsigned_abs())
            .unwrap_or(params.len())
    } else {
        (offset as usize).saturating_sub(1)
    };
    let take = match length {
        Some(length) if length < 0 => params
            .len()
            .saturating_sub(start)
            .saturating_sub(length.unsigned_abs()),
        Some(length) => usize::try_from(length).unwrap_or(usize::MAX),
        None => usize::MAX,
    };

    params.iter().skip(start).take(take).cloned().collect()
}

pub(in crate::executor) fn parse_parameter_replacement(
    name: &str,
) -> Option<(&str, &str, &str, bool)> {
    if let Some((var_name, rest)) = name.split_once("//") {
        let (pattern, replacement) = rest.split_once('/').unwrap_or((rest, ""));
        return Some((var_name, pattern, replacement, true));
    }

    let (var_name, rest) = name.split_once('/')?;
    let (pattern, replacement) = rest.split_once('/').unwrap_or((rest, ""));
    Some((var_name, pattern, replacement, false))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn matching_parameter_brace_skips_escaped_closing_brace() {
        let input = "foo:-string \\\\\\}}";

        assert_eq!(matching_parameter_brace(input), Some(input.len() - 1));
        assert!(braced_parameter_spans_whole_word("${foo:-string \\\\\\}}"));
    }

    #[test]
    fn matching_parameter_brace_closes_after_even_backslashes() {
        let input = "foo:-string \\\\}}";

        assert_eq!(matching_parameter_brace(input), Some(input.len() - 2));
    }

    #[test]
    fn matching_parameter_brace_ignores_closing_brace_in_bracket_pattern() {
        assert_eq!(matching_parameter_brace("o%[}]}"), Some(5));
    }

    #[test]
    fn replacement_decoder_preserves_backslashes() {
        assert_eq!(decode_parameter_replacement_quotes(r"\n"), "n");
        assert_eq!(decode_parameter_replacement_quotes(r"\\n"), r"\n");
        assert_eq!(decode_parameter_replacement_quotes(r"\&"), r"\&");
    }
}
