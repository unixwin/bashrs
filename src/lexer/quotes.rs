use super::ansi::decode_ansi_c_quoted;
use super::dolbrace::{scan_braced_parameter, BraceContext, DolbraceState};

pub(crate) fn remove_shell_quotes(raw: &str) -> String {
    let mut out = String::new();
    let mut chars = raw.chars().peekable();

    while let Some(ch) = chars.next() {
        match ch {
            '$' if chars.peek() == Some(&'(') => {
                copy_dollar_paren_substitution(&mut out, &mut chars);
            }
            '$' if chars.peek() == Some(&'\'') => {
                chars.next();
                let mut quoted = String::new();
                let mut escaped = false;
                for quoted_ch in chars.by_ref() {
                    if escaped {
                        quoted.push('\\');
                        quoted.push(quoted_ch);
                        escaped = false;
                        continue;
                    }
                    if quoted_ch == '\\' {
                        escaped = true;
                        continue;
                    }
                    if quoted_ch == '\'' {
                        break;
                    }
                    quoted.push(quoted_ch);
                }
                if escaped {
                    quoted.push('\\');
                }
                out.push_str(&decode_ansi_c_quoted(&quoted));
            }
            '$' if chars.peek() == Some(&'"') => {
                chars.next();
                remove_double_quoted_into(&mut out, &mut chars, false);
            }
            '$' if chars.peek() == Some(&'{') => {
                copy_braced_parameter_unquoted(&mut out, &mut chars);
            }
            '\'' => {
                for quoted in chars.by_ref() {
                    if quoted == '\'' {
                        break;
                    }
                    if quoted == '$' {
                        // Preserve the existing protected-dollar contract used by
                        // downstream expansion, but do not protect literal globs.
                        out.push('\x1f');
                    } else {
                        out.push(quoted);
                    }
                }
            }
            '"' => {
                remove_double_quoted_into(&mut out, &mut chars, false);
            }
            '`' => {
                out.push(ch);
                copy_backtick_body_preserving_syntax(&mut out, &mut chars);
            }
            '\\' => {
                let Some(escaped) = chars.next() else {
                    out.push(ch);
                    continue;
                };
                if escaped == '$' {
                    out.push('\x1f');
                } else if escaped == '`' {
                    out.push('\x1a');
                } else if escaped == '\'' {
                    out.push('\x17');
                } else if escaped == '\\' {
                    // Keep a literal backslash distinct from the protected
                    // double-quote marker used by expansion internals.
                    out.push('\x14');
                } else if matches!(escaped, '*' | '?' | '[' | '@' | '+' | '!') {
                    out.push('\x11');
                    out.push(escaped);
                } else {
                    out.push(escaped);
                }
            }
            _ => out.push(ch),
        }
    }

    out
}

pub(super) fn remove_shell_quotes_outside_backticks(raw: &str) -> String {
    let mut out = String::new();
    let mut chars = raw.chars().peekable();

    while let Some(ch) = chars.next() {
        match ch {
            '`' => {
                out.push(ch);
                copy_backtick_body_preserving_syntax(&mut out, &mut chars);
            }
            '$' if chars.peek() == Some(&'"') => {
                chars.next();
                remove_double_quoted_into(&mut out, &mut chars, true);
            }
            '$' if chars.peek() == Some(&'{') => {
                copy_braced_parameter_unquoted(&mut out, &mut chars);
            }
            '\'' => {
                for quoted in chars.by_ref() {
                    if quoted == '\'' {
                        break;
                    }
                    out.push(quoted);
                }
            }
            '"' => {
                remove_double_quoted_into(&mut out, &mut chars, true);
            }
            '\\' => {
                let Some(escaped) = chars.next() else {
                    out.push(ch);
                    continue;
                };
                if escaped == '\'' {
                    out.push('\x17');
                } else if escaped == '`' {
                    out.push('\x1a');
                } else {
                    out.push(escaped);
                }
            }
            _ => out.push(ch),
        }
    }

    out
}

pub(super) fn normalize_backtick_command_substitution(raw: &str) -> String {
    let mut chars = raw.chars().peekable();
    if chars.next() != Some('`') {
        return raw.to_string();
    }
    let mut out = String::from("`");
    copy_backtick_body_preserving_syntax(&mut out, &mut chars);
    out.extend(chars);
    out
}

fn remove_double_quoted_into(
    out: &mut String,
    chars: &mut std::iter::Peekable<std::str::Chars<'_>>,
    preserve_backticks: bool,
) {
    while let Some(quoted) = chars.next() {
        if quoted == '$' && chars.peek() == Some(&'(') {
            copy_dollar_paren_substitution(out, chars);
            continue;
        }
        if quoted == '$' && chars.peek() == Some(&'{') {
            copy_braced_parameter_after_dollar(out, chars);
            continue;
        }
        if quoted == '$'
            && matches!(
                chars.peek().copied(),
                Some('?' | '$' | '!' | '#' | '-' | '@' | '*' | '0'..='9')
            )
        {
            out.push('$');
            if let Some(param) = chars.next() {
                out.push(param);
            }
            continue;
        }
        match quoted {
            '"' => break,
            '`' if preserve_backticks => {
                out.push(quoted);
                copy_backtick_body_preserving_syntax(out, chars);
            }
            '\\' => {
                if let Some(escaped @ ('\\' | '"' | '$' | '`' | '\n')) = chars.peek().copied() {
                    chars.next();
                    if escaped != '\n' {
                        match escaped {
                            '$' => out.push('\x1f'),
                            '`' => out.push('\x1a'),
                            '\\' => out.push('\x14'),
                            _ => out.push(escaped),
                        }
                    }
                } else {
                    out.push('\\');
                }
            }
            '\'' => {
                // GNU parse.y skip_double_quoted: only ", \, $ and ` are
                // special inside double quotes; a single quote is an ordinary
                // literal character. Carry it with the same protected-literal
                // marker as \' so the expansion stage keeps it as data instead
                // of re-reading it as a single-quote delimiter (which would
                // also suppress parameter expansion across the pseudo span).
                out.push('\x17');
            }
            _ if matches!(quoted, '*' | '?' | '[' | '@' | '+' | '!') => {
                out.push('\x11');
                out.push(quoted);
            }
            _ => out.push(quoted),
        }
    }
}

fn copy_dollar_paren_substitution(
    out: &mut String,
    chars: &mut std::iter::Peekable<std::str::Chars<'_>>,
) {
    out.push('$');
    if chars.next() != Some('(') {
        return;
    }
    out.push('(');
    copy_dollar_paren_body_raw(out, chars);
}

fn copy_single_quoted_raw(out: &mut String, chars: &mut std::iter::Peekable<std::str::Chars<'_>>) {
    for ch in chars.by_ref() {
        out.push(ch);
        if ch == '\'' {
            break;
        }
    }
}

fn copy_ansi_c_single_quoted_raw(
    out: &mut String,
    chars: &mut std::iter::Peekable<std::str::Chars<'_>>,
) {
    let mut escaped = false;
    for ch in chars.by_ref() {
        out.push(ch);
        if escaped {
            escaped = false;
            continue;
        }
        if ch == '\\' {
            escaped = true;
            continue;
        }
        if ch == '\'' {
            break;
        }
    }
}

fn copy_double_quoted_raw(out: &mut String, chars: &mut std::iter::Peekable<std::str::Chars<'_>>) {
    while let Some(ch) = chars.next() {
        out.push(ch);
        match ch {
            '"' => break,
            '\\' => {
                if let Some(escaped) = chars.next() {
                    out.push(escaped);
                }
            }
            '$' if chars.peek() == Some(&'(') => {
                chars.next();
                out.push('(');
                copy_dollar_paren_body_raw(out, chars);
            }
            _ => {}
        }
    }
}

fn copy_dollar_paren_body_raw(
    out: &mut String,
    chars: &mut std::iter::Peekable<std::str::Chars<'_>>,
) {
    let mut depth = 1usize;
    while let Some(ch) = chars.next() {
        out.push(ch);
        match ch {
            '$' if chars.peek() == Some(&'\'') => {
                chars.next();
                out.push('\'');
                copy_ansi_c_single_quoted_raw(out, chars);
            }
            '$' if chars.peek() == Some(&'(') => {
                chars.next();
                out.push('(');
                depth += 1;
            }
            '\'' => copy_single_quoted_raw(out, chars),
            '"' => copy_double_quoted_raw(out, chars),
            '`' => copy_backtick_raw(out, chars),
            '\\' => {
                if let Some(escaped) = chars.next() {
                    out.push(escaped);
                }
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

fn copy_backtick_raw(out: &mut String, chars: &mut std::iter::Peekable<std::str::Chars<'_>>) {
    copy_backtick_body_preserving_syntax(out, chars);
}

fn copy_backtick_body_preserving_syntax(
    out: &mut String,
    chars: &mut std::iter::Peekable<std::str::Chars<'_>>,
) {
    while let Some(ch) = chars.next() {
        if ch == '`' {
            out.push(ch);
            break;
        }
        if ch == '\\' {
            match chars.next() {
                Some('\n') => {}
                Some('\r') if chars.peek().copied() == Some('\n') => {
                    chars.next();
                }
                Some(escaped) => {
                    out.push(ch);
                    out.push(escaped);
                }
                None => out.push(ch),
            }
            continue;
        }
        out.push(ch);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn preserves_single_quotes_inside_double_quoted_parameter_word() {
        assert_eq!(remove_shell_quotes("\"${IFS+'}'z}\""), "${IFS+'}'z}");
    }

    #[test]
    fn double_quoted_single_quote_becomes_protected_literal_data() {
        // GNU parse.y skip_double_quoted: a single quote inside double
        // quotes is ordinary data, carried with the same protected marker
        // as an escaped quote so expansion never re-reads it as a
        // single-quote delimiter.
        assert_eq!(
            remove_shell_quotes("\"a:'b' c\""),
            "a:\x17b\x17 c"
        );
    }

}

// Source-mapped to subst.c::extract_dollar_brace_string: quote removal
// receives explicit outer-quote and POSIX context instead of conflating them.
pub(super) fn copy_braced_parameter_after_dollar(
    out: &mut String,
    chars: &mut std::iter::Peekable<std::str::Chars<'_>>,
) {
    out.push('$');
    if chars.peek() != Some(&'{') {
        return;
    }
    let remaining: String = chars.clone().collect();
    let mut wrapped = String::from("$");
    wrapped.push_str(&remaining);
    let context = BraceContext {
        outer_double_quote: true,
        // The surrounding double quote is not POSIX mode.
        posix: false,
        replacement_context: false,
        initial_state: DolbraceState::Param,
    };
    if let Some(scan) = scan_braced_parameter(&wrapped, context) {
        let consumed = wrapped[..scan.end].chars().count().saturating_sub(1);
        for _ in 0..consumed {
            if let Some(ch) = chars.next() {
                out.push(ch);
            }
        }
        return;
    }
    out.push(chars.next().unwrap());
    let mut depth = 1usize;
    while let Some(ch) = chars.next() {
        out.push(ch);
        if ch == '$' && chars.peek() == Some(&'{') {
            chars.next();
            out.push('{');
            depth += 1;
            continue;
        }
        if ch == '}' {
            depth = depth.saturating_sub(1);
            if depth == 0 {
                break;
            }
        }
    }
}

// Unquoted `${...}` units keep their body verbatim (GNU quote removal never
// strips quotes inside a parameter expansion; the expansion stage owns the
// quote state). Whole-word `${...}` tokens already bypass quote removal via
// the lexer's Variable path; this gives embedded occurrences the same shape.
fn copy_braced_parameter_unquoted(
    out: &mut String,
    chars: &mut std::iter::Peekable<std::str::Chars<'_>>,
) {
    out.push('$');
    if chars.peek() != Some(&'{') {
        return;
    }
    let remaining: String = chars.clone().collect();
    let mut wrapped = String::from("$");
    wrapped.push_str(&remaining);
    let context = BraceContext {
        outer_double_quote: false,
        posix: false,
        replacement_context: false,
        initial_state: DolbraceState::Param,
    };
    if let Some(scan) = scan_braced_parameter(&wrapped, context) {
        let consumed = wrapped[..scan.end].chars().count().saturating_sub(1);
        for _ in 0..consumed {
            if let Some(ch) = chars.next() {
                out.push(ch);
            }
        }
        return;
    }
    out.push(chars.next().unwrap());
    let mut depth = 1usize;
    while let Some(ch) = chars.next() {
        out.push(ch);
        if ch == '$' && chars.peek() == Some(&'{') {
            chars.next();
            out.push('{');
            depth += 1;
            continue;
        }
        if ch == '}' {
            depth = depth.saturating_sub(1);
            if depth == 0 {
                break;
            }
        }
    }
}
