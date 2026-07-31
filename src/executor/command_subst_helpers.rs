pub(in crate::executor) fn collect_braced_parameter_name(
    chars: &mut std::iter::Peekable<std::str::Chars<'_>>,
) -> String {
    let mut name = String::new();
    let mut nested = 0usize;
    while let Some(ch) = chars.next() {
        if ch == '$' && chars.peek().copied() == Some('{') {
            chars.next();
            nested += 1;
            name.push('$');
            name.push('{');
            continue;
        }
        if ch == '}' {
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
                | '<' | '>' | '!' | '*' | '?' | '#'),
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
    String::from_utf8_lossy(&bytes)
        .trim_end_matches('\n')
        .to_string()
}

pub(in crate::executor) fn echo_raw_output(args: &[String]) -> String {
    String::from_utf8_lossy(&echo_raw_output_bytes(args)).into_owned()
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
}
