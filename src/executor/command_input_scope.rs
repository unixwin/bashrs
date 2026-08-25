use super::*;

impl Executor {
    pub(crate) fn with_command_input_redirects<T>(
        &mut self,
        cmd: &CommandNode,
        execute: impl FnOnce(&mut Executor) -> Result<T, ExecuteError>,
    ) -> Result<T, ExecuteError> {
        let Some(input) = self.command_input_redirect(cmd) else {
            return execute(self);
        };

        let old_function_stdin = self.env_vars.get(FUNCTION_STDIN).cloned();
        let old_function_stdin_offset = self.env_vars.get(FUNCTION_STDIN_OFFSET).cloned();
        self.env_vars.insert(FUNCTION_STDIN.to_string(), input);
        self.env_vars
            .insert(FUNCTION_STDIN_OFFSET.to_string(), "0".to_string());

        let result = execute(self);
        restore_optional_env_var(&mut self.env_vars, FUNCTION_STDIN, old_function_stdin);
        restore_optional_env_var(
            &mut self.env_vars,
            FUNCTION_STDIN_OFFSET,
            old_function_stdin_offset,
        );
        result
    }

    pub(in crate::executor) fn command_input_redirect(
        &mut self,
        cmd: &CommandNode,
    ) -> Option<String> {
        if let Some(input) = self.loop_redirect_input(cmd) {
            return Some(input);
        }

        if let Some(here_string) = &cmd.here_string {
            return Some(self.expand_word(here_string));
        }

        if let Some(heredoc) = &cmd.heredoc {
            return Some(self.expand_heredoc_body(heredoc));
        }

        cmd.heredoc_redirects
            .iter()
            .rev()
            .find(|redirect| redirect.fd.is_none())
            .and_then(|redirect| redirect.body.as_deref())
            .map(|body| self.expand_heredoc_body(body))
    }

    /// Expands an unquoted heredoc body like Bash: parameter, command and
    /// arithmetic expansions run in the context of the receiving command.
    /// A `\x1e` prefix marks a quoted heredoc whose body stays literal.
    pub(in crate::executor) fn expand_heredoc_body(&self, body: &str) -> String {
        let quoted = body.starts_with(crate::lexer::QUOTED_HEREDOC_MARKER);
        let body = strip_unterminated_heredoc_marker(strip_quoted_heredoc_marker(body));
        if quoted {
            return body.to_string();
        }
        let expanded = self.expand_embedded_parameters_for_heredoc(
            &prepare_unquoted_heredoc_expansion(body),
        );
        decode_command_substitution_payload(&restore_command_substitution_output(&expanded))
    }
}

fn prepare_unquoted_heredoc_expansion(body: &str) -> String {
    let mut output = String::with_capacity(body.len());
    let mut chars = body.chars().peekable();
    let mut command_depth = 0usize;
    let mut single = false;
    let mut double = false;
    while let Some(ch) = chars.next() {
        if ch == '$' && !single && chars.peek() == Some(&'(') {
            chars.next();
            output.push('$');
            output.push('(');
            command_depth += 1;
            continue;
        }

        if command_depth > 0 {
            match ch {
                '\'' if !double => single = !single,
                '"' if !single => double = !double,
                '(' if !single && !double => command_depth += 1,
                ')' if !single && !double => command_depth = command_depth.saturating_sub(1),
                _ => {}
            }
        }

        if ch != '\\' {
            output.push(ch);
            continue;
        }

        if command_depth > 0 && single {
            output.push('\\');
            continue;
        }

        let mut slash_count = 1usize;
        while chars.peek() == Some(&'\\') {
            chars.next();
            slash_count += 1;
        }

        if matches!(chars.peek(), Some('\n') | Some('\r')) {
            for _ in 0..(slash_count / 2) {
                output.push('\\');
            }
            if slash_count % 2 == 1 {
                if chars.peek() == Some(&'\r') {
                    chars.next();
                }
                if chars.peek() == Some(&'\n') {
                    chars.next();
                }
            }
            continue;
        }

        for _ in 0..(slash_count / 2) {
            output.push('\x14');
        }
        if slash_count % 2 == 0 {
            continue;
        }

        match chars.peek().copied() {
            Some('$') => {
                chars.next();
                output.push('\x1f');
            }
            Some('`') => {
                chars.next();
                output.push('\x1a');
            }
            Some('\\') => unreachable!("backslash runs are consumed above"),
            _ => output.push('\\'),
        }
    }
    output
}

#[allow(dead_code)]
fn strip_heredoc_body(body: &str) -> String {
    strip_unterminated_heredoc_marker(strip_quoted_heredoc_marker(body)).to_string()
}
