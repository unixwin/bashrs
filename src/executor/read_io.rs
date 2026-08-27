use super::*;
use crate::executor::substitution_metadata::bytes_to_shell_text;

impl Executor {
    pub(in crate::executor) fn finish_read_error(
        &mut self,
        cmd: &CommandNode,
        stderr: &[u8],
        status: i32,
    ) -> i32 {
        self.write_buffered_builtin_output(cmd, &[], stderr)
            .map(|_| status)
            .unwrap_or(1)
    }

    pub(in crate::executor) fn continue_read_line_after_backslash(
        &mut self,
        cmd: &CommandNode,
        read_fd: Option<u32>,
        mut line: String,
    ) -> String {
        while line.ends_with('\\') {
            line.pop();
            let Some(next) = self.read_input_for_command(cmd, read_fd, '\n', None, false) else {
                break;
            };
            line.push_str(&next);
        }
        line
    }

    pub(in crate::executor) fn read_input_for_command(
        &mut self,
        cmd: &CommandNode,
        read_fd: Option<u32>,
        delimiter: char,
        char_limit: Option<usize>,
        exact_char_limit: bool,
    ) -> Option<String> {
        // An unnumbered heredoc is the last stdin redirect, so it overrides
        // an earlier `<&fd`. An explicit `read -u N` still owns the input fd.
        if read_fd.is_none() {
            if let Some(heredoc) = &cmd.heredoc {
                return Some(trim_read_input(
                    self.expand_heredoc_body_mut(heredoc),
                    delimiter,
                    char_limit,
                    exact_char_limit,
                ));
            }
        }

        if let Some(fd) = read_fd {
            if let Some(line) =
                self.read_redirected_fd(cmd, fd, delimiter, char_limit, exact_char_limit)
            {
                return Some(line);
            }
            if let Some(output) =
                self.read_coproc_stdout(fd, delimiter, char_limit, exact_char_limit)
            {
                return Some(trim_read_input(
                    output,
                    delimiter,
                    char_limit,
                    exact_char_limit,
                ));
            }
            return self
                .read_virtual_fd_stdin(fd, delimiter, char_limit, exact_char_limit)
                .or_else(|| {
                    self.read_heredoc_fd_input(cmd, fd, delimiter, char_limit, exact_char_limit)
                });
        }

        if let Some(redirect) = &cmd.redirect_in {
            if redirect.fd.unwrap_or(0) != 0 {
                return None;
            }
            if is_closed_redirect_target(&self.expand_word(&redirect.target)) {
                return None;
            }
            if let Some(source) = redirect
                .target
                .strip_prefix("<(")
                .and_then(|target| target.strip_suffix(')'))
            {
                if let Some(output) = self.process_substitution_output(source) {
                    return Some(trim_read_input(
                        output,
                        delimiter,
                        char_limit,
                        exact_char_limit,
                    ));
                }
            }

            let expanded_target = self.expand_word(&redirect.target);
            if let Some(fd) = expanded_target.strip_prefix('&') {
                let fd = fd.trim_matches(|ch| ch == '"' || ch == '\x1d');
                if let Ok(fd) = fd.parse::<u32>() {
                    if let Some(output) =
                        self.read_coproc_stdout(fd, delimiter, char_limit, exact_char_limit)
                    {
                        return Some(output);
                    }
                    if let Some(line) =
                        self.read_virtual_fd_stdin(fd, delimiter, char_limit, exact_char_limit)
                    {
                        return Some(line);
                    }
                    return self.read_heredoc_fd_input(
                        cmd,
                        fd,
                        delimiter,
                        char_limit,
                        exact_char_limit,
                    );
                }
            }

            let path = shell_path_to_windows(&expanded_target, &self.env_vars);
            if redirect.append {
                let _ = OpenOptions::new()
                    .create(true)
                    .read(true)
                    .write(true)
                    .open(&path);
            }
            // GNU read receives raw bytes from redir.c-opened inputs. Keep
            // invalid UTF-8 inside the RAW_BYTE_MARKER carrier instead of
            // dropping the whole record at this boundary.
            let Ok(input) =
                crate::executor::substitution_metadata::read_shell_input_file(path)
            else {
                return None;
            };
            if input.is_empty() {
                return None;
            }
            return Some(trim_read_input(
                input,
                delimiter,
                char_limit,
                exact_char_limit,
            ));
        }

        // Here-strings are command-local input. Persistent virtual fds must
        // bypass `stdin_string_for_command`, whose legacy remaining-text view
        // does not advance the shared fd cursor.
        if cmd.here_string.is_some() {
            if let Some(line) = self.stdin_string_for_command_mut(cmd) {
                return Some(trim_read_input(
                    line,
                    delimiter,
                    char_limit,
                    exact_char_limit,
                ));
            }
        }

        // Persistent fd 0 owns the shared cursor. The legacy mirror can be
        // present for external setup, but must not reset a shell read to offset 0.
        if matches!(
            self.fd_table.read_endpoint(0),
            Some(FdReadEndpoint::Text(_) | FdReadEndpoint::ProcessSubstitution(_))
        ) {
            if let Some(line) =
                self.read_virtual_fd_stdin(0, delimiter, char_limit, exact_char_limit)
            {
                return Some(line);
            }
        }

        // If FUNCTION_STDIN is set (from heredoc or redirect), only read from it.
        // Do NOT fall through to process stdin - that would block on the terminal.
        if self.env_vars.contains_key(FUNCTION_STDIN) {
            return self.read_function_stdin(delimiter, char_limit, exact_char_limit);
        }

        if let Some(line) = self.read_virtual_fd_stdin(0, delimiter, char_limit, exact_char_limit) {
            return Some(line);
        }

        if self.fd_table.is_closed(0) {
            return None;
        }

        self.read_function_stdin(delimiter, char_limit, exact_char_limit)
            .or_else(|| self.read_inherited_process_stdin(delimiter, char_limit, exact_char_limit))
    }

    fn read_coproc_stdout(
        &mut self,
        fd: u32,
        delimiter: char,
        char_limit: Option<usize>,
        exact_char_limit: bool,
    ) -> Option<String> {
        // Bash exposes the coprocess output as COPROC[0] (and NAME[0]).
        // Rubash stores that endpoint as a PipeReader keyed by the child PID.
        // A zero descriptor retains the legacy unnamed-coproc behavior; named
        // coprocess arrays carry their PID as a virtual descriptor.
        if self.coproc_stdout_readers.is_empty() {
            return None;
        }
        let pid = if fd == 0 {
            *self.coproc_stdout_readers.keys().next()?
        } else if let Some(FdReadEndpoint::CoprocStdout(pid)) = self.fd_table.read_endpoint(fd) {
            pid
        } else {
            fd
        };
        if !self.coproc_stdout_readers.contains_key(&pid) {
            return None;
        }
        let mut reader = self.coproc_stdout_readers.remove(&pid)?;
        let mut bytes = Vec::new();
        let mut consumed_chars = 0usize;
        let mut ended = false;
        use std::io::Read;

        // Bash keeps the coprocess descriptor open across read builtin calls.
        // Read only one logical input record (or the requested character
        // limit), then retain the reader for the next call instead of
        // draining the pipe and losing unread records.
        loop {
            let mut byte = [0u8; 1];
            match reader.read(&mut byte) {
                Ok(0) => {
                    ended = true;
                    break;
                }
                Ok(_) => {
                    bytes.push(byte[0]);
                    if byte[0] == delimiter as u8 && !exact_char_limit {
                        break;
                    }
                    if let Some(limit) = char_limit {
                        consumed_chars += 1;
                        if consumed_chars >= limit {
                            break;
                        }
                    }
                }
                Err(_) => {
                    ended = true;
                    break;
                }
            }
        }

        if !ended {
            self.coproc_stdout_readers.insert(pid, reader);
        } else if matches!(
            self.fd_table.read_endpoint(fd),
            Some(FdReadEndpoint::CoprocStdout(_))
        ) {
            // EOF closes this shell-owned read capability. Job reaping remains
            // separate so wait can still consume the child's final status.
            self.fd_table.close_input(fd);
        }
        if bytes.is_empty() {
            return None;
        }
        Some(trim_read_input(
            String::from_utf8_lossy(&bytes).to_string(),
            delimiter,
            char_limit,
            exact_char_limit,
        ))
    }

    pub(crate) fn process_substitution_output(&mut self, source: &str) -> Option<String> {
        self.process_substitution_output_bytes(source)
            .map(|output| bytes_to_shell_text(&output))
    }

    pub(crate) fn process_substitution_output_bytes(&mut self, source: &str) -> Option<Vec<u8>> {
        let tokens = crate::lexer::tokenize(source);
        let ast = crate::parser::parse(&tokens);
        if ast.commands.is_empty() {
            return None;
        }

        let saved_dir = env::current_dir().ok();
        let mut subshell = self.command_substitution_executor();
        crate::builtins::trap::reset_for_subshell(&mut subshell.env_vars);
        subshell.stdout_capture = Some(Vec::new());
        let result = subshell.execute_ast(&ast);
        let output = subshell.stdout_capture.take().unwrap_or_default();

        if let Some(saved_dir) = saved_dir {
            let _ = env::set_current_dir(saved_dir);
        }

        match result {
            Ok(()) | Err(ExecuteError::ExitCode(_)) | Err(ExecuteError::Return(_)) => Some(output),
            Err(_) => None,
        }
    }

    pub(in crate::executor) fn read_virtual_fd_stdin(
        &mut self,
        fd: u32,
        delimiter: char,
        char_limit: Option<usize>,
        exact_char_limit: bool,
    ) -> Option<String> {
        if self.fd_table.is_open_for_read(fd) {
            if let Some(line) = self
                .fd_table
                .read_text(fd, delimiter, char_limit, exact_char_limit)
            {
                if let Some((_, offset)) = self.fd_table.input_snapshot(fd) {
                    self.env_vars
                        .insert(fd_stdin_offset_key(fd), offset.to_string());
                }
                return Some(trim_read_input(
                    line,
                    delimiter,
                    char_limit,
                    exact_char_limit,
                ));
            }
            if self.fd_table.is_closed(fd) {
                return None;
            }
        }
        // Coprocess readers are stream endpoints, not text mirrors. Once the
        // pipe reaches EOF, do not interpret the legacy environment adapter
        // value as shell input.
        if matches!(
            self.fd_table.read_endpoint(fd),
            Some(FdReadEndpoint::CoprocStdout(_))
        ) {
            return None;
        }
        if matches!(
            self.fd_table.read_endpoint(fd),
            Some(FdReadEndpoint::InheritedProcessStdin)
        ) {
            return self.read_inherited_process_stdin(delimiter, char_limit, exact_char_limit);
        }
        None
    }

    pub(in crate::executor) fn read_function_stdin(
        &mut self,
        delimiter: char,
        char_limit: Option<usize>,
        exact_char_limit: bool,
    ) -> Option<String> {
        let input = self.env_vars.get(FUNCTION_STDIN)?.clone();
        let offset = self
            .env_vars
            .get(FUNCTION_STDIN_OFFSET)
            .and_then(|value| value.parse::<usize>().ok())
            .unwrap_or(0);
        if offset >= input.len() {
            return None;
        }
        if char_limit == Some(0) {
            return Some(String::new());
        }

        let slice = &input[offset..];
        let mut output = String::new();
        let mut consumed = 0usize;
        let mut took_any = false;
        for (index, ch) in slice.char_indices() {
            if !exact_char_limit && ch == delimiter {
                consumed = index + ch.len_utf8();
                took_any = true;
                break;
            }

            output.push(ch);
            consumed = index + ch.len_utf8();
            took_any = true;
            if char_limit.is_some_and(|limit| output.chars().count() >= limit) {
                break;
            }
        }
        if !took_any {
            return None;
        }

        self.env_vars.insert(
            FUNCTION_STDIN_OFFSET.to_string(),
            (offset + consumed).to_string(),
        );
        Some(trim_read_input(
            output,
            delimiter,
            char_limit,
            exact_char_limit,
        ))
    }

    pub(in crate::executor) fn read_inherited_process_stdin(
        &self,
        delimiter: char,
        char_limit: Option<usize>,
        exact_char_limit: bool,
    ) -> Option<String> {
        if self.env_vars.get(INHERIT_PROCESS_STDIN).map(String::as_str) != Some("1") {
            return None;
        }
        if char_limit == Some(0) {
            return Some(String::new());
        }

        let mut stdin = io::stdin().lock();
        let mut bytes = [0_u8; 1];
        let mut output = String::new();
        loop {
            let count = stdin.read(&mut bytes).ok()?;
            if count == 0 {
                break;
            }

            let ch = bytes[0] as char;
            if !exact_char_limit && ch == delimiter {
                break;
            }

            output.push(ch);
            if char_limit.is_some_and(|limit| output.chars().count() >= limit) {
                break;
            }
        }

        if output.is_empty() {
            return None;
        }

        Some(trim_read_input(
            output,
            delimiter,
            char_limit,
            exact_char_limit,
        ))
    }

    pub(in crate::executor) fn read_inherited_process_stdin_to_string(&self) -> Option<String> {
        if self.env_vars.get(INHERIT_PROCESS_STDIN).map(String::as_str) != Some("1") {
            return None;
        }

        let mut stdin = io::stdin().lock();
        let mut output = String::new();
        stdin.read_to_string(&mut output).ok()?;
        if output.is_empty() {
            return None;
        }
        Some(output)
    }
}
