use crate::executor::substitution_metadata::bytes_to_shell_text;

pub(in crate::executor) fn collect_braced_parameter_name(
    chars: &mut std::iter::Peekable<std::str::Chars<'_>>,
) -> String {
    let mut name = String::new();
    let mut nested = 0usize;
    let mut in_bracket_expression = false;
    while let Some(ch) = chars.next() {
        // GNU Bash scans `\` plus the following character as one unit
        // (extract_dollar_brace_string advances by two). `\\` is a literal
        // backslash, `\}` a literal closing brace; neither closes the name.
        if ch == '\\' {
            if let Some(escaped) = chars.next() {
                name.push('\\');
                name.push(escaped);
            } else {
                name.push('\\');
            }
            continue;
        }
        if ch == '[' {
            in_bracket_expression = true;
            name.push(ch);
            continue;
        }
        if ch == ']' && in_bracket_expression {
            in_bracket_expression = false;
            name.push(ch);
            continue;
        }
        if ch == '$' && chars.peek().copied() == Some('{') {
            chars.next();
            nested += 1;
            name.push('$');
            name.push('{');
            continue;
        }
        if ch == '}' && !in_bracket_expression {
            if nested == 0 {
                break;
            }
            nested -= 1;
            name.push(ch);
            continue;
        }
        name.push(ch);
    }
    name
}

pub(in crate::executor) fn decode_old_style_backtick_source(source: &str) -> String {
    let mut output = String::new();
    let mut chars = source.chars().peekable();
    let mut single = false;
    let mut double = false;
    while let Some(ch) = chars.next() {
        if ch == '\'' && !double {
            single = !single;
            output.push(ch);
            continue;
        }

        if ch == '"' && !single {
            double = !double;
            output.push(ch);
            continue;
        }

        if ch != '\\' {
            push_backtick_source_char(&mut output, ch, single);
            continue;
        }

        if double {
            let mut lookahead = chars.clone();
            if lookahead.next() == Some('\\') && lookahead.next() == Some('"') {
                chars.next();
                chars.next();
                output.push('\x18');
                continue;
            }
        }

        if double && chars.peek().copied() == Some('"') {
            chars.next();
            output.push('\x18');
            continue;
        }

        match chars.next() {
            Some(next @ ('$' | '`' | '\\')) => {
                push_backtick_source_char(&mut output, next, single);
            }
            Some('\n') => {}
            Some('\r') if chars.peek().copied() == Some('\n') => {
                chars.next();
            }
            Some(next) => {
                output.push('\\');
                push_backtick_source_char(&mut output, next, single);
            }
            None => output.push('\\'),
        }
    }
    output
}

fn push_backtick_source_char(output: &mut String, ch: char, single: bool) {
    if single && ch == '$' {
        output.push('\x1f');
    } else {
        output.push(ch);
    }
}

pub(super) fn unescape_remaining_shell_escapes(value: &str) -> String {
    let mut output = String::new();
    let mut chars = value.chars().peekable();
    while let Some(ch) = chars.next() {
        if ch == '\\' {
            let mut lookahead = chars.clone();
            if lookahead.next() == Some('\\') && lookahead.next() == Some('\'') {
                chars.next();
                chars.next();
                output.push('\'');
                continue;
            }
            if let Some(
                next @ ('\'' | '"' | '\\' | '$' | '`' | '(' | ')' | '{' | '}' | ';' | '&' | '|'
                | '<' | '>' | '!' | '*' | '?' | '#' | ' '),
            ) = chars.peek().copied()
            {
                chars.next();
                output.push(next);
                continue;
            }
        }
        output.push(ch);
    }
    output
}

pub(in crate::executor) fn echo_command_substitution_output(args: &[String]) -> String {
    let mut bytes = echo_raw_output_bytes(args);
    bytes.retain(|byte| *byte != 0);
    bytes_to_shell_text(&bytes)
        .trim_end_matches('\n')
        .to_string()
}

pub(in crate::executor) fn echo_raw_output(args: &[String]) -> String {
    bytes_to_shell_text(&echo_raw_output_bytes(args))
}

fn echo_raw_output_bytes(args: &[String]) -> Vec<u8> {
    let mut output = Vec::new();
    let _ = crate::builtins::echo::write_echo(args.iter().map(String::as_str), &mut output);
    output
}

pub(in crate::executor) fn split_pipeline_words(words: &[String]) -> Option<Vec<&[String]>> {
    let mut stages = Vec::new();
    let mut start = 0usize;
    for (index, word) in words.iter().enumerate() {
        if word == "|" {
            if start == index {
                return None;
            }
            stages.push(&words[start..index]);
            start = index + 1;
        }
    }
    if start >= words.len() {
        return None;
    }
    stages.push(&words[start..]);
    (stages.len() > 1).then_some(stages)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn args(values: &[&str]) -> Vec<String> {
        values.iter().map(|value| value.to_string()).collect()
    }

    #[test]
    fn echo_command_substitution_uses_builtin_escape_rules() {
        assert_eq!(
            echo_command_substitution_output(&args(&["-e", "a\\nb"])),
            "a\nb"
        );
        assert_eq!(
            echo_command_substitution_output(&args(&["-e", "\\xz"])),
            "\\xz"
        );
        assert_eq!(
            echo_command_substitution_output(&args(&["--help"])),
            "--help"
        );
    }

    #[test]
    fn echo_command_substitution_removes_nul_bytes() {
        assert_eq!(
            echo_command_substitution_output(&args(&["-e", "a\\0b"])),
            "ab"
        );
    }

    #[test]
    fn echo_raw_output_keeps_builtin_stdout_shape() {
        assert_eq!(echo_raw_output(&args(&["-n", "hello"])), "hello");
        assert_eq!(echo_raw_output(&args(&["hello"])), "hello\n");
    }

    #[test]
    fn braced_parameter_collection_ignores_braces_in_pattern_classes() {
        let mut chars = "o%[}]}]".chars().peekable();
        assert_eq!(collect_braced_parameter_name(&mut chars), "o%[}]");
        assert_eq!(chars.collect::<String>(), "]");
    }
}
