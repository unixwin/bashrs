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
        let quoted = body.starts_with('\x1e');
        let body = strip_unterminated_heredoc_marker(strip_quoted_heredoc_marker(body));
        if quoted {
            return body.to_string();
        }
        let source = prepare_unquoted_heredoc_expansion(body);
        restore_heredoc_expansion_markers(&self.expand_embedded_parameters(&source))
    }

    pub(in crate::executor) fn unquoted_heredoc_expansion_source(
        &self,
        body: &str,
    ) -> Option<String> {
        let quoted = body.starts_with('\x1e');
        let body = strip_unterminated_heredoc_marker(strip_quoted_heredoc_marker(body));
        (!quoted).then(|| prepare_unquoted_heredoc_expansion(body))
    }
}

fn prepare_unquoted_heredoc_expansion(body: &str) -> String {
    let mut output = String::with_capacity(body.len());
    let mut chars = body.chars().peekable();
    let mut dollar_paren_depth = 0usize;
    let mut single_in_dollar_paren = false;
    while let Some(ch) = chars.next() {
        if dollar_paren_depth > 0 {
            if ch == '\'' {
                single_in_dollar_paren = !single_in_dollar_paren;
                output.push(ch);
                continue;
            }
            if single_in_dollar_paren {
                output.push(ch);
                continue;
            }
            if ch == ')' {
                dollar_paren_depth = dollar_paren_depth.saturating_sub(1);
                output.push(ch);
                continue;
            }
        }

        if ch == '$' && chars.peek().copied() == Some('(') {
            chars.next();
            dollar_paren_depth += 1;
            output.push('$');
            output.push('(');
            continue;
        }

        if ch != '\\' {
            output.push(ch);
            continue;
        }
        match chars.peek().copied() {
            Some('\n') => {
                chars.next();
            }
            Some('\\') => {
                chars.next();
                output.push('\x15');
            }
            Some('$') => {
                chars.next();
                output.push('\x1f');
            }
            Some('`') => {
                chars.next();
                output.push('\x1a');
            }
            _ => output.push('\\'),
        }
    }
    output
}

fn restore_heredoc_expansion_markers(value: &str) -> String {
    value
        .replace('\x1a', "`")
        .replace('\x1f', "$")
        .replace('\x15', "\\")
}

#[allow(dead_code)]
fn strip_heredoc_body(body: &str) -> String {
    strip_unterminated_heredoc_marker(strip_quoted_heredoc_marker(body)).to_string()
}
