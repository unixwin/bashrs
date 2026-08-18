use super::*;

impl Executor {
    pub(in crate::executor) fn set_fd_input_text(&mut self, fd: u32, input: String, dynamic: bool) {
        self.fd_table
            .open_input(fd, FdReadEndpoint::text(&input), dynamic);
        self.env_vars.insert(fd_stdin_key(fd), input);
        self.env_vars
            .insert(fd_stdin_offset_key(fd), "0".to_string());
        if dynamic {
            self.env_vars
                .insert(fd_dynamic_input_key(fd), "1".to_string());
        } else {
            self.env_vars.remove(&fd_dynamic_input_key(fd));
        }
        self.env_vars.remove(&fd_closed_key(fd));
    }

    fn set_fd_output_file(&mut self, fd: u32, target: String, dynamic: bool) {
        let path = shell_path_to_windows(&target, &self.env_vars);
        self.fd_table
            .open_output(fd, FdWriteEndpoint::File(path), dynamic);
        self.env_vars.remove(&fd_closed_key(fd));
        self.env_vars
            .remove(&fd_output_process_substitution_key(fd));
        self.env_vars.insert(fd_output_key(fd), target);
    }

    pub(in crate::executor) fn execute_eval(
        &mut self,
        cmd: &CommandNode,
    ) -> Result<(), ExecuteError> {
        let mut stderr = Vec::new();
        let args = cmd.words[1..].to_vec();
        match crate::builtins::eval::execute_with_io(args.iter().map(String::as_str), &mut stderr)?
        {
            crate::builtins::eval::EvalAction::Complete(status) => {
                self.write_buffered_builtin_output(cmd, &[], &stderr)?;
                self.exit_code = status;
                Ok(())
            }
            crate::builtins::eval::EvalAction::Execute(source) => {
                self.write_buffered_builtin_output(cmd, &[], &stderr)?;
                let source = eval_source_for_reparse(&source);
                let tokens = crate::lexer::tokenize(&source);
                let mut ast = crate::parser::parse(&tokens);
                self.apply_command_output_redirects(cmd, &mut ast)?;
                self.execute_ast(&ast)
            }
        }
    }

    pub fn run_exit_trap(&mut self) -> Result<i32, ExecuteError> {
        self.run_exit_trap_for_status(self.exit_code)
    }

    pub fn run_exit_trap_with_status(&mut self, exit_status: i32) -> Result<i32, ExecuteError> {
        self.run_exit_trap_for_status(exit_status)
    }

    pub(in crate::executor) fn run_exit_trap_for_status(
        &mut self,
        exit_status: i32,
    ) -> Result<i32, ExecuteError> {
        let Some(action) = crate::builtins::trap::take_exit_trap(&mut self.env_vars) else {
            return Ok(exit_status);
        };
        if action.is_empty() {
            return Ok(exit_status);
        }

        self.exit_code = exit_status;
        let tokens = crate::lexer::tokenize(&action);
        let ast = crate::parser::parse(&tokens);
        let saved_trap_command = self.debug_trap_command.clone();
        if self.debug_trap_command.is_none() {
            self.debug_trap_command = self
                .env_vars
                .get("__RUBASH_LAST_COMMAND")
                .or_else(|| self.env_vars.get("__RUBASH_CURRENT_COMMAND"))
                .cloned();
        }
        let result = self.execute_ast(&ast);
        self.debug_trap_command = saved_trap_command;
        match result {
            Ok(()) => {
                self.exit_code = exit_status;
                Ok(exit_status)
            }
            Err(ExecuteError::ExitCode(code)) => {
                self.exit_code = code;
                Ok(code)
            }
            Err(error) => Err(error),
        }
    }

    /// Runs the DEBUG trap action before a command, mirroring Bash's
    /// per-command debug hook. Nested executions of the trap action are
    /// suppressed (Bash does not re-enter the DEBUG trap while an action
    /// is running). `command_text` is the text of the command about to run,
    /// exposed to the trap action through BASH_COMMAND like Bash does.
    pub(crate) fn run_debug_trap(&mut self, command_text: &str) -> Result<bool, ExecuteError> {
        if self.debug_trap_running {
            return Ok(false);
        }
        let Some(action) = crate::builtins::trap::get_trap_action(&self.env_vars, "DEBUG") else {
            return Ok(false);
        };
        if action.is_empty() {
            return Ok(false);
        }
        self.debug_trap_running = true;
        self.debug_trap_command = Some(command_text.to_string());
        let call_line = self
            .env_vars
            .get("__RUBASH_CURRENT_LINE")
            .and_then(|line| line.parse::<usize>().ok());
        let tokens = crate::lexer::tokenize(&action);
        let mut ast = crate::parser::parse(&tokens);
        if let Some(call_line) = call_line {
            for command in &mut ast.commands {
                command.line = Some(call_line);
            }
        }
        let result = self.execute_ast(&ast);
        self.debug_trap_command = None;
        self.debug_trap_running = false;
        result?;
        let skip_command = self.exit_code == 2;
        if skip_command {
            self.exit_code = 0;
        }
        Ok(skip_command)
    }

    /// Runs the RETURN trap action when a function (or sourced script)
    /// returns. Mirrors Bash's `trap ... RETURN` hook used by debuggers.
    pub(crate) fn run_return_trap(&mut self) -> Result<(), ExecuteError> {
        if self.return_trap_running {
            return Ok(());
        }
        let Some(action) = crate::builtins::trap::get_trap_action(&self.env_vars, "RETURN") else {
            return Ok(());
        };
        if action.is_empty() {
            return Ok(());
        }
        self.return_trap_running = true;
        let tokens = crate::lexer::tokenize(&action);
        let ast = crate::parser::parse(&tokens);
        let result = self.execute_ast(&ast);
        self.return_trap_running = false;
        result
    }

    pub(crate) fn run_pending_signal_traps(&mut self) -> Result<(), ExecuteError> {
        if self.signal_trap_running {
            return Ok(());
        }

        let signals = crate::builtins::kill::take_pending_signals(std::process::id())?;
        for signal in signals {
            let Some(signal_name) = signal_trap_name(signal) else {
                continue;
            };
            let action = crate::builtins::trap::get_trap_action(&self.env_vars, &signal_name);
            let Some(action) = action else {
                // Bash's default disposition for SIGCHLD is to ignore it.
                // Child completion/reaping notifications must not turn into
                // a synthetic 128+SIGCHLD shell exit when no CHLD trap is
                // installed (busybox ash `reap*.tests`).
                if signal == 20 {
                    continue;
                }
                return Err(ExecuteError::ExitCode(128 + signal));
            };
            if action.is_empty() {
                continue;
            }

            let saved_exit = self.exit_code;
            self.exit_code = saved_exit;
            self.signal_trap_running = true;
            let old_signal_status = self.env_vars.insert(
                "__RUBASH_SIGNAL_TRAP_STATUS".to_string(),
                saved_exit.to_string(),
            );
            let tokens = crate::lexer::tokenize(&action);
            let ast = crate::parser::parse(&tokens);
            let result = self.execute_ast(&ast);
            match old_signal_status {
                Some(value) => {
                    self.env_vars
                        .insert("__RUBASH_SIGNAL_TRAP_STATUS".to_string(), value);
                }
                None => {
                    self.env_vars.remove("__RUBASH_SIGNAL_TRAP_STATUS");
                }
            }
            self.signal_trap_running = false;
            match result {
                Ok(()) => self.exit_code = saved_exit,
                Err(error @ ExecuteError::Return(_)) => return Err(error),
                Err(error @ ExecuteError::ExitCode(_)) => return Err(error),
                Err(error) => return Err(error),
            }
        }

        Ok(())
    }

    pub(crate) fn run_function_return_trap(&mut self) -> Result<(), ExecuteError> {
        // Bash only fires a RETURN trap for function returns when tracing is
        // enabled (`set -T`) or `extdebug` is active.  Sourced files use the
        // unconditional run_return_trap path above.
        let traced = crate::builtins::set::shell_option_enabled(&self.env_vars, "functrace")
            || crate::builtins::shopt::option_enabled(&self.env_vars, "extdebug");
        let function_scoped = self
            .env_vars
            .get("__RUBASH_RETURN_TRAP_FUNCTION")
            .zip(self.function_name_stack.first())
            .is_some_and(|(registered, current)| registered == current);
        if !traced && !function_scoped {
            return Ok(());
        }
        self.run_return_trap()
    }

    pub(in crate::executor) fn note_return_trap_scope(&mut self, args: &[String]) {
        let mut index = usize::from(args.first().map(String::as_str) == Some("--"));
        let Some(action) = args.get(index) else {
            return;
        };
        if action != "-" && action.starts_with('-') {
            return;
        }
        index += 1;
        let has_return = args[index..].iter().any(|signal| {
            signal
                .strip_prefix("SIG")
                .unwrap_or(signal)
                .eq_ignore_ascii_case("RETURN")
        });
        if !has_return {
            return;
        }
        if action == "-" {
            self.env_vars.remove("__RUBASH_RETURN_TRAP_FUNCTION");
        } else if let Some(function) = self.function_name_stack.first() {
            self.env_vars.insert(
                "__RUBASH_RETURN_TRAP_FUNCTION".to_string(),
                function.clone(),
            );
        }
    }

    pub(crate) fn apply_command_output_redirects(
        &mut self,
        cmd: &CommandNode,
        ast: &mut Ast,
    ) -> Result<(), ExecuteError> {
        if let Some(redirect) = &cmd.redirect_out {
            let target = self.expand_word(&redirect.target);
            if !is_closed_redirect_target(&target) && redirect_target_fd(&target).is_none() {
                self.create_redirect_output(&target, redirect.clobber)?;
            }
            let append_redirect = Redirect {
                fd: redirect.fd,
                fd_var: redirect.fd_var.clone(),
                operator: ">>".to_string(),
                operator_metadata: Box::new(crate::parser::WordMetadata::new(
                    0,
                    ">>".to_string(),
                    ">>".to_string(),
                )),
                kind: crate::parser::RedirectKind::Append,
                target_metadata: Box::new(crate::parser::WordMetadata::new(
                    0,
                    target.clone(),
                    target.clone(),
                )),
                target,
                append: true,
                clobber: false,
            };
            apply_stdout_append_redirect(&mut ast.commands, &append_redirect);
        } else if let Some(redirect) = &cmd.append {
            let target = self.expand_word(&redirect.target);
            let append_redirect = Redirect {
                fd: redirect.fd,
                fd_var: redirect.fd_var.clone(),
                operator: redirect.operator.clone(),
                operator_metadata: redirect.operator_metadata.clone(),
                kind: redirect.kind.clone(),
                target_metadata: Box::new(crate::parser::WordMetadata::new(
                    0,
                    target.clone(),
                    target.clone(),
                )),
                target,
                append: true,
                clobber: false,
            };
            apply_stdout_append_redirect(&mut ast.commands, &append_redirect);
        }

        if let Some(redirect) = &cmd.redirect_err {
            let target = self.expand_word(&redirect.target);
            if !is_closed_redirect_target(&target)
                && redirect_target_fd(&target).is_none()
                && !is_null_device(&target)
            {
                self.create_redirect_output(&target, redirect.clobber)?;
            }
            let append_redirect = Redirect {
                fd: redirect.fd,
                fd_var: redirect.fd_var.clone(),
                operator: "2>>".to_string(),
                operator_metadata: Box::new(crate::parser::WordMetadata::new(
                    0,
                    "2>>".to_string(),
                    "2>>".to_string(),
                )),
                kind: crate::parser::RedirectKind::Append,
                target_metadata: Box::new(crate::parser::WordMetadata::new(
                    0,
                    target.clone(),
                    target.clone(),
                )),
                target,
                append: true,
                clobber: false,
            };
            apply_stderr_append_redirect(&mut ast.commands, &append_redirect);
        } else if let Some(redirect) = &cmd.redirect_err_append {
            let target = self.expand_word(&redirect.target);
            let append_redirect = Redirect {
                fd: redirect.fd,
                fd_var: redirect.fd_var.clone(),
                operator: redirect.operator.clone(),
                operator_metadata: redirect.operator_metadata.clone(),
                kind: redirect.kind.clone(),
                target_metadata: Box::new(crate::parser::WordMetadata::new(
                    0,
                    target.clone(),
                    target.clone(),
                )),
                target,
                append: true,
                clobber: false,
            };
            apply_stderr_append_redirect(&mut ast.commands, &append_redirect);
        }

        Ok(())
    }

    pub(in crate::executor) fn execute_exec(
        &mut self,
        cmd: &CommandNode,
    ) -> Result<i32, ExecuteError> {
        if let Some(status) = self.execute_dynamic_fd_exec_redirect(cmd)? {
            return Ok(status);
        }

        if exec_has_only_redirects(cmd) {
            if let Some(status) = self.execute_stdio_only_exec_redirect(cmd)? {
                return Ok(status);
            }
        }

        // Bash applies redirections before `exec` parses its options.  This
        // matters when an expanded option is invalid: the diagnostic is still
        // emitted, but a stdout redirection remains in effect afterwards.
        if self.exec_has_no_command_operand_after_expansion(cmd) {
            self.execute_stdio_only_exec_redirect(cmd)?;
            let mut stdout = Vec::new();
            let mut stderr = Vec::new();
            let status = crate::builtins::exec::execute_with_io(
                &cmd.words[1..],
                &self.env_vars,
                &mut stdout,
                &mut stderr,
            )?;
            self.write_buffered_builtin_output(cmd, &stdout, &stderr)?;
            return Ok(status);
        }

        if let Some(redirect) = &cmd.redirect_out {
            let target = self.expand_word(&redirect.target);
            let mut file = self.create_redirect_output(&target, redirect.clobber)?;
            return Ok(crate::builtins::exec::execute_with_io(
                &cmd.words[1..],
                &self.env_vars,
                &mut file,
                &mut std::io::stderr().lock(),
            )?);
        }

        if let Some(redirect) = &cmd.append {
            let target = self.expand_word(&redirect.target);
            let mut file = OpenOptions::new()
                .create(true)
                .append(true)
                .open(shell_path_to_windows(&target, &self.env_vars))?;
            return Ok(crate::builtins::exec::execute_with_io(
                &cmd.words[1..],
                &self.env_vars,
                &mut file,
                &mut std::io::stderr().lock(),
            )?);
        }

        if let Some(redirect) = &cmd.redirect_err {
            let target = self.expand_word(&redirect.target);
            let mut file = self.create_redirect_output(&target, redirect.clobber)?;
            return Ok(crate::builtins::exec::execute_with_io(
                &cmd.words[1..],
                &self.env_vars,
                &mut std::io::stdout().lock(),
                &mut file,
            )?);
        }

        if let Some(redirect) = &cmd.redirect_err_append {
            let target = self.expand_word(&redirect.target);
            let mut file = OpenOptions::new()
                .create(true)
                .append(true)
                .open(shell_path_to_windows(&target, &self.env_vars))?;
            return Ok(crate::builtins::exec::execute_with_io(
                &cmd.words[1..],
                &self.env_vars,
                &mut std::io::stdout().lock(),
                &mut file,
            )?);
        }

        self.apply_no_output_builtin_redirects(cmd)?;
        Ok(crate::builtins::exec::execute(
            &cmd.words[1..],
            &self.env_vars,
        )?)
    }

    fn execute_stdio_only_exec_redirect(
        &mut self,
        cmd: &CommandNode,
    ) -> Result<Option<i32>, ExecuteError> {
        // Fd-prefixed redirects retain the raw process-substitution target.
        for redirect in &cmd.redirects {
            let target = &redirect.target;
            let fd = redirect.fd.unwrap_or(1);
            match redirect.kind {
                crate::parser::RedirectKind::Output
                | crate::parser::RedirectKind::Append
                | crate::parser::RedirectKind::ClobberOutput
                    if target.starts_with(">(") && target.ends_with(')') =>
                {
                    self.open_persistent_output_process_substitution(fd, target)?;
                    return Ok(Some(0));
                }
                _ => {}
            }
        }

        if let Some(redirect) = &cmd.redirect_out {
            let target = self.expand_word(&redirect.target);
            let fd = redirect.fd.unwrap_or(1);
            if is_closed_redirect_target(&target) {
                if let Some(name) = redirect.fd_var.as_deref() {
                    self.close_dynamic_fd(name)?;
                } else {
                    self.close_persistent_output_fd(fd)?;
                    self.env_vars.insert(fd_closed_key(fd), "1".to_string());
                }
                return Ok(Some(0));
            }
            if let Some((source_fd, move_source)) = redirect_target_fd_and_move(&target) {
                self.copy_persistent_output_fd(fd, source_fd);
                if move_source {
                    self.close_persistent_output_fd(source_fd)?;
                }
                return Ok(Some(0));
            }
            if let Some(source_fd) = redirect_target_fd(&target) {
                self.copy_persistent_output_fd(fd, source_fd);
                return Ok(Some(0));
            }
            if self.open_persistent_output_process_substitution(fd, &target)? {
                return Ok(Some(0));
            }
            self.create_redirect_output(&target, redirect.clobber)?;
            self.set_fd_output_file(fd, target, fd >= 10);
            return Ok(Some(0));
        }

        if let Some(redirect) = &cmd.append {
            let target = self.expand_word(&redirect.target);
            let fd = redirect.fd.unwrap_or(1);
            if is_closed_redirect_target(&target) {
                self.close_persistent_output_fd(fd)?;
                self.env_vars.insert(fd_closed_key(fd), "1".to_string());
                return Ok(Some(0));
            }
            if self.open_persistent_output_process_substitution(fd, &target)? {
                return Ok(Some(0));
            }
            OpenOptions::new()
                .create(true)
                .append(true)
                .open(shell_path_to_windows(&target, &self.env_vars))?;
            self.set_fd_output_file(fd, target, fd >= 10);
            return Ok(Some(0));
        }

        if let Some(redirect) = &cmd.redirect_err {
            let target = self.expand_word(&redirect.target);
            let fd = redirect.fd.unwrap_or(2);
            if is_closed_redirect_target(&target) {
                self.close_persistent_output_fd(fd)?;
                self.env_vars.insert(fd_closed_key(fd), "1".to_string());
                return Ok(Some(0));
            }
            if let Some(source_fd) = redirect_target_fd(&target) {
                self.copy_persistent_output_fd(fd, source_fd);
                return Ok(Some(0));
            }
            if self.open_persistent_output_process_substitution(fd, &target)? {
                return Ok(Some(0));
            }
            if !is_null_device(&target) {
                self.create_redirect_output(&target, redirect.clobber)?;
            }
            self.set_fd_output_file(fd, target, fd >= 10);
            return Ok(Some(0));
        }

        if let Some(redirect) = &cmd.redirect_err_append {
            let target = self.expand_word(&redirect.target);
            let fd = redirect.fd.unwrap_or(2);
            if is_closed_redirect_target(&target) {
                self.close_persistent_output_fd(fd)?;
                self.env_vars.insert(fd_closed_key(fd), "1".to_string());
                return Ok(Some(0));
            }
            if self.open_persistent_output_process_substitution(fd, &target)? {
                return Ok(Some(0));
            }
            if !is_null_device(&target) {
                OpenOptions::new()
                    .create(true)
                    .append(true)
                    .open(shell_path_to_windows(&target, &self.env_vars))?;
            }
            self.set_fd_output_file(fd, target, fd >= 10);
            return Ok(Some(0));
        }

        if let Some(redirect) = &cmd.redirect_in {
            let fd = redirect.fd.unwrap_or(0);
            let target = self.expand_word(&redirect.target);
            if is_closed_redirect_target(&target) {
                self.close_persistent_input_fd(fd);
                self.env_vars.insert(fd_closed_key(fd), "1".to_string());
                return Ok(Some(0));
            }
            if let Some((source_fd, move_source)) = redirect_target_fd_and_move(&target) {
                self.copy_persistent_input_fd(fd, source_fd);
                if move_source {
                    self.close_persistent_input_fd(source_fd);
                }
                return Ok(Some(0));
            }
            if let Some(source_fd) = redirect_target_fd(&target) {
                self.copy_persistent_input_fd(fd, source_fd);
                return Ok(Some(0));
            }

            if let Some(source) = target
                .strip_prefix("<(")
                .and_then(|target| target.strip_suffix(')'))
            {
                if let Some(input) = self.process_substitution_output(source) {
                    self.fd_table.open_input(
                        fd,
                        FdReadEndpoint::process_substitution(&input),
                        fd != 0,
                    );
                    self.set_fd_input_text(fd, input, fd != 0);
                    return Ok(Some(0));
                }
            }

            if matches!(
                target.as_str(),
                "/dev/stdin" | "/proc/self/fd/0" | "/dev/fd/0"
            ) {
                self.fd_table
                    .open_input(fd, FdReadEndpoint::InheritedProcessStdin, fd != 0);
                return Ok(Some(0));
            }

            let path = shell_path_to_windows(&target, &self.env_vars);
            if redirect.append {
                let _ = OpenOptions::new()
                    .create(true)
                    .read(true)
                    .write(true)
                    .open(&path)?;
            }
            let input = fs::read_to_string(&path)?;
            self.set_fd_input_text(fd, input, fd != 0);
            if redirect.operator == "<>" {
                self.fd_table
                    .open_output(fd, FdWriteEndpoint::File(path), fd >= 10);
            }
            return Ok(Some(0));
        }

        if let Some((fd, input)) = self.exec_heredoc_fd_input(cmd) {
            self.set_fd_input_text(fd, input, fd != 0);
            return Ok(Some(0));
        }

        Ok(None)
    }

    fn exec_heredoc_fd_input(&self, cmd: &CommandNode) -> Option<(u32, String)> {
        let redirect = cmd
            .heredoc_redirects
            .iter()
            .rev()
            .find(|redirect| redirect.fd.is_some() && redirect.body.is_some())?;
        let fd = redirect.fd?;
        let body = redirect.body.as_deref()?;
        let input = if let Some(word) = body.strip_prefix('\x1d') {
            let mut input =
                decode_ansi_c_quoted_word(word).unwrap_or_else(|| self.expand_word(word));
            input.push('\n');
            input
        } else {
            strip_unterminated_heredoc_marker(strip_quoted_heredoc_marker(body)).to_string()
        };
        Some((fd, input))
    }

    fn copy_persistent_output_fd(&mut self, target_fd: u32, source_fd: u32) {
        if self.fd_table.is_open_for_write(source_fd) {
            let source_endpoint = self.fd_table.write_endpoint(source_fd);
            let target_endpoint = self.fd_table.write_endpoint(target_fd);
            if target_fd != source_fd
                && target_endpoint.as_ref().is_some_and(|endpoint| {
                    matches!(endpoint, FdWriteEndpoint::ProcessSubstitution { .. })
                })
                && target_endpoint != source_endpoint
            {
                let _ = self.close_persistent_output_fd(target_fd);
            }
            if self.fd_table.dup_output(target_fd, source_fd).is_ok() {
                match self.fd_table.output_endpoint(target_fd) {
                    Some(FdWriteEndpoint::Stdout) => {
                        self.env_vars.remove(&fd_closed_key(target_fd));
                        self.env_vars
                            .insert(fd_output_key(target_fd), FD_STDOUT_TARGET.to_string());
                        self.env_vars
                            .remove(&fd_output_process_substitution_key(target_fd));
                    }
                    Some(FdWriteEndpoint::Stderr) => {
                        self.env_vars.remove(&fd_closed_key(target_fd));
                        self.env_vars
                            .insert(fd_output_key(target_fd), FD_STDERR_TARGET.to_string());
                        self.env_vars
                            .remove(&fd_output_process_substitution_key(target_fd));
                    }
                    Some(FdWriteEndpoint::File(path)) => {
                        self.env_vars.remove(&fd_closed_key(target_fd));
                        self.env_vars.insert(
                            fd_output_key(target_fd),
                            shell_display_path(&path.to_string_lossy()),
                        );
                        self.env_vars
                            .remove(&fd_output_process_substitution_key(target_fd));
                    }
                    Some(FdWriteEndpoint::CoprocStdin(pid)) => {
                        self.env_vars.remove(&fd_closed_key(target_fd));
                        self.env_vars.insert(
                            fd_output_key(target_fd),
                            format!("{FD_COPROC_STDIN_TARGET_PREFIX}{pid}"),
                        );
                        self.env_vars
                            .remove(&fd_output_process_substitution_key(target_fd));
                    }
                    Some(FdWriteEndpoint::ProcessSubstitution { path, command }) => {
                        self.env_vars.remove(&fd_closed_key(target_fd));
                        self.env_vars.insert(
                            fd_output_key(target_fd),
                            shell_display_path(&path.to_string_lossy()),
                        );
                        self.env_vars
                            .insert(fd_output_process_substitution_key(target_fd), command);
                    }
                    None => {}
                }
            } else {
                let _ = self.close_persistent_output_fd(target_fd);
                self.env_vars
                    .insert(fd_closed_key(target_fd), "1".to_string());
            }
            return;
        }
        if self.env_vars.contains_key(&fd_closed_key(source_fd)) {
            let _ = self.close_persistent_output_fd(target_fd);
            self.env_vars
                .insert(fd_closed_key(target_fd), "1".to_string());
        } else if self.coproc_stdin_writers.contains_key(&source_fd) {
            self.env_vars.remove(&fd_closed_key(target_fd));
            self.env_vars.insert(
                fd_output_key(target_fd),
                format!("{FD_COPROC_STDIN_TARGET_PREFIX}{source_fd}"),
            );
            self.env_vars
                .remove(&fd_output_process_substitution_key(target_fd));
        } else if let Some(target) = self.env_vars.get(&fd_output_key(source_fd)).cloned() {
            self.env_vars.remove(&fd_closed_key(target_fd));
            self.env_vars.insert(fd_output_key(target_fd), target);
            if let Some(source) = self
                .env_vars
                .get(&fd_output_process_substitution_key(source_fd))
                .cloned()
            {
                self.env_vars
                    .insert(fd_output_process_substitution_key(target_fd), source);
            } else {
                self.env_vars
                    .remove(&fd_output_process_substitution_key(target_fd));
            }
        } else if let Some(target) = stdio_output_target(source_fd) {
            self.env_vars.remove(&fd_closed_key(target_fd));
            self.env_vars
                .insert(fd_output_key(target_fd), target.to_string());
            self.env_vars
                .remove(&fd_output_process_substitution_key(target_fd));
        } else {
            let _ = self.close_persistent_output_fd(target_fd);
            self.env_vars.remove(&fd_closed_key(target_fd));
        }
    }

    fn open_persistent_output_process_substitution(
        &mut self,
        fd: u32,
        target: &str,
    ) -> Result<bool, ExecuteError> {
        let Some(source) = target
            .strip_prefix(">(")
            .and_then(|target| target.strip_suffix(')'))
        else {
            return Ok(false);
        };

        let path = self.empty_process_substitution_temp()?;
        self.fd_table.open_output(
            fd,
            FdWriteEndpoint::ProcessSubstitution {
                path: path.clone(),
                command: source.to_string(),
            },
            fd >= 10,
        );
        self.env_vars.remove(&fd_closed_key(fd));
        self.env_vars
            .insert(fd_output_key(fd), path.to_string_lossy().into_owned());
        self.env_vars
            .insert(fd_output_process_substitution_key(fd), source.to_string());
        Ok(true)
    }

    fn close_persistent_output_fd(&mut self, fd: u32) -> Result<(), ExecuteError> {
        let coproc_pid = self
            .fd_table
            .entries
            .get(&fd)
            .filter(|entry| !entry.closed)
            .and_then(|entry| match entry.write.as_ref() {
                Some(FdWriteEndpoint::CoprocStdin(pid)) => Some(*pid),
                _ => None,
            });
        self.fd_table.close_output(fd);
        if let Some(pid) = coproc_pid {
            let has_alias = self.fd_table.entries.values().any(|entry| {
                !entry.closed
                    && matches!(
                        entry.write.as_ref(),
                        Some(FdWriteEndpoint::CoprocStdin(alias_pid))
                            if *alias_pid == pid
                    )
            });
            if !has_alias {
                self.coproc_stdin_writers.remove(&pid);
            }
        }
        let target = self.env_vars.remove(&fd_output_key(fd));
        let source = self
            .env_vars
            .remove(&fd_output_process_substitution_key(fd));
        if let (Some(target), Some(source)) = (target, source) {
            let path = shell_path_to_windows(&target, &self.env_vars);
            let input = fs::read_to_string(&path).unwrap_or_default();
            self.execute_persistent_output_process_substitution(&source, input)?;
            let _ = fs::remove_file(path);
        }
        Ok(())
    }

    fn close_persistent_input_fd(&mut self, fd: u32) {
        let coproc_pid = self
            .fd_table
            .read_endpoint(fd)
            .and_then(|endpoint| match endpoint {
                FdReadEndpoint::CoprocStdout(pid) => Some(pid),
                _ => None,
            });
        self.fd_table.close_input(fd);
        if let Some(pid) = coproc_pid {
            let has_alias = self.fd_table.entries.values().any(|entry| {
                !entry.closed
                    && matches!(
                        entry.read.as_ref(),
                        Some(FdReadEndpoint::CoprocStdout(alias_pid))
                            if *alias_pid == pid
                    )
            });
            if !has_alias {
                self.coproc_stdout_readers.remove(&pid);
            }
        }
        self.env_vars.remove(&fd_stdin_key(fd));
        self.env_vars.remove(&fd_stdin_offset_key(fd));
        self.env_vars.remove(&fd_dynamic_input_key(fd));
    }

    fn close_persistent_fd(&mut self, fd: u32) -> Result<(), ExecuteError> {
        self.close_persistent_output_fd(fd)?;
        self.close_persistent_input_fd(fd);
        self.fd_table.close(fd);
        self.env_vars.insert(fd_closed_key(fd), "1".to_string());
        Ok(())
    }

    pub(in crate::executor) fn execute_persistent_output_process_substitution(
        &mut self,
        source: &str,
        input: String,
    ) -> Result<(), ExecuteError> {
        let tokens = crate::lexer::tokenize(source);
        let ast = crate::parser::parse(&tokens);
        let old_stdin = self.env_vars.get(FUNCTION_STDIN).cloned();
        let old_offset = self.env_vars.get(FUNCTION_STDIN_OFFSET).cloned();
        let old_fd0 = self.fd_table.entries.get(&0).cloned();
        let fd0_key = fd_stdin_key(0);
        let fd0_offset_key = fd_stdin_offset_key(0);
        let fd0_dynamic_key = fd_dynamic_input_key(0);
        let fd0_closed_key = fd_closed_key(0);
        let old_fd0_stdin = self.env_vars.get(&fd0_key).cloned();
        let old_fd0_offset = self.env_vars.get(&fd0_offset_key).cloned();
        let old_fd0_dynamic = self.env_vars.get(&fd0_dynamic_key).cloned();
        let old_fd0_closed = self.env_vars.get(&fd0_closed_key).cloned();
        self.set_fd_input_text(0, input.clone(), false);
        self.env_vars.insert(FUNCTION_STDIN.to_string(), input);
        self.env_vars
            .insert(FUNCTION_STDIN_OFFSET.to_string(), "0".to_string());
        let result = self.execute_ast(&ast);
        match old_fd0 {
            Some(entry) => {
                self.fd_table.entries.insert(0, entry);
            }
            None => {
                self.fd_table.entries.remove(&0);
            }
        }
        restore_optional_env_var(&mut self.env_vars, &fd0_key, old_fd0_stdin);
        restore_optional_env_var(&mut self.env_vars, &fd0_offset_key, old_fd0_offset);
        restore_optional_env_var(&mut self.env_vars, &fd0_dynamic_key, old_fd0_dynamic);
        restore_optional_env_var(&mut self.env_vars, &fd0_closed_key, old_fd0_closed);
        restore_optional_env_var(&mut self.env_vars, FUNCTION_STDIN, old_stdin);
        restore_optional_env_var(&mut self.env_vars, FUNCTION_STDIN_OFFSET, old_offset);
        result
    }

    fn execute_dynamic_fd_exec_redirect(
        &mut self,
        cmd: &CommandNode,
    ) -> Result<Option<i32>, ExecuteError> {
        let Some(name) = cmd.words.get(1).and_then(|word| dynamic_fd_var_name(word)) else {
            return Ok(None);
        };
        if cmd.words.len() != 2 {
            return Ok(None);
        }

        if cmd.here_string.is_some() || cmd.heredoc.is_some() {
            let Some(input) = self.stdin_string_for_command(cmd) else {
                return Ok(None);
            };
            let fd = self.allocate_dynamic_fd();
            self.set_dynamic_fd_variable(name, fd);
            self.set_fd_input_text(fd, input, true);
            return Ok(Some(0));
        }

        if let Some(redirect) = &cmd.redirect_in {
            let target = self.expand_word(&redirect.target);
            if is_closed_redirect_target(&target) {
                self.close_dynamic_fd(name)?;
                return Ok(Some(0));
            }

            if let Some((source_fd, move_source)) = redirect_target_fd_and_move(&target) {
                let fd = self.allocate_dynamic_fd();
                self.set_dynamic_fd_variable(name, fd);
                self.copy_persistent_input_fd(fd, source_fd);
                if move_source {
                    self.close_persistent_fd(source_fd)?;
                }
                return Ok(Some(0));
            }

            if let Some(source) = target
                .strip_prefix("<(")
                .and_then(|target| target.strip_suffix(')'))
            {
                if let Some(input) = self.process_substitution_output(source) {
                    let fd = self.allocate_dynamic_fd();
                    self.set_dynamic_fd_variable(name, fd);
                    self.fd_table.open_input(
                        fd,
                        FdReadEndpoint::process_substitution(&input),
                        true,
                    );
                    self.set_fd_input_text(fd, input, true);
                    return Ok(Some(0));
                }
            }

            if matches!(
                target.as_str(),
                "/dev/stdin" | "/proc/self/fd/0" | "/dev/fd/0"
            ) {
                let fd = self.allocate_dynamic_fd();
                self.set_dynamic_fd_variable(name, fd);
                self.fd_table
                    .open_input(fd, FdReadEndpoint::InheritedProcessStdin, true);
                return Ok(Some(0));
            }

            let path = shell_path_to_windows(&target, &self.env_vars);
            if redirect.append {
                let _ = OpenOptions::new()
                    .create(true)
                    .read(true)
                    .write(true)
                    .open(&path)?;
            }
            let input = fs::read_to_string(path)?;
            let fd = self.allocate_dynamic_fd();
            self.set_dynamic_fd_variable(name, fd);
            self.set_fd_input_text(fd, input, true);
            if redirect.operator == "<>" {
                self.set_fd_output_file(fd, target, true);
            }
            return Ok(Some(0));
        }

        if let Some(redirect) = &cmd.redirect_out {
            let target = self.expand_word(&redirect.target);
            if is_closed_redirect_target(&target) {
                self.close_dynamic_output_fd(name)?;
                return Ok(Some(0));
            }

            let fd = self.allocate_dynamic_fd();
            self.set_dynamic_fd_variable(name, fd);
            if let Some((source_fd, move_source)) = redirect_target_fd_and_move(&target) {
                self.copy_persistent_output_fd(fd, source_fd);
                if move_source {
                    self.close_persistent_fd(source_fd)?;
                }
                return Ok(Some(0));
            }
            if self.open_persistent_output_process_substitution(fd, &target)? {
                return Ok(Some(0));
            }
            self.create_redirect_output(&target, redirect.clobber)?;
            self.set_fd_output_file(fd, target, true);
            return Ok(Some(0));
        }

        if let Some(redirect) = &cmd.append {
            let target = self.expand_word(&redirect.target);
            if is_closed_redirect_target(&target) {
                self.close_dynamic_output_fd(name)?;
                return Ok(Some(0));
            }
            let fd = self.allocate_dynamic_fd();
            if self.open_persistent_output_process_substitution(fd, &target)? {
                self.set_dynamic_fd_variable(name, fd);
                return Ok(Some(0));
            }
            OpenOptions::new()
                .create(true)
                .append(true)
                .open(shell_path_to_windows(&target, &self.env_vars))?;
            self.set_dynamic_fd_variable(name, fd);
            self.set_fd_output_file(fd, target, true);
            return Ok(Some(0));
        }

        Ok(None)
    }

    pub(in crate::executor) fn execute_dynamic_fd_var_redirect(
        &mut self,
        redirect: &Redirect,
        auto_close: bool,
    ) -> Result<bool, ExecuteError> {
        let Some(name) = redirect.fd_var.as_deref() else {
            return Ok(false);
        };
        let target = self.expand_word(&redirect.target);
        let close_after_command =
            auto_close && crate::builtins::shopt::option_enabled(&self.env_vars, "varredir_close");

        let close_after_success = |executor: &mut Self, fd: u32| -> Result<(), ExecuteError> {
            if close_after_command {
                executor.close_persistent_fd(fd)?;
            }
            Ok(())
        };

        match redirect.kind {
            crate::parser::RedirectKind::CloseInput => {
                self.close_dynamic_input_fd(name);
                return Ok(true);
            }
            crate::parser::RedirectKind::CloseOutput => {
                self.close_dynamic_output_fd(name)?;
                return Ok(true);
            }
            crate::parser::RedirectKind::DuplicateInput => {
                if let Some((source_fd, move_source)) = redirect_target_fd_and_move(&target) {
                    if !self.fd_table.is_open_for_read(source_fd) {
                        return Ok(true);
                    }
                    let fd = self.allocate_dynamic_fd();
                    self.copy_persistent_input_fd(fd, source_fd);
                    self.set_dynamic_fd_variable(name, fd);
                    if move_source {
                        self.close_persistent_input_fd(source_fd);
                    }
                    close_after_success(self, fd)?;
                    return Ok(true);
                }
            }
            crate::parser::RedirectKind::DuplicateOutput => {
                if let Some((source_fd, move_source)) = redirect_target_fd_and_move(&target) {
                    if !self.fd_table.is_open_for_write(source_fd) {
                        return Ok(true);
                    }
                    let fd = self.allocate_dynamic_fd();
                    self.copy_persistent_output_fd(fd, source_fd);
                    self.set_dynamic_fd_variable(name, fd);
                    if move_source {
                        self.close_persistent_output_fd(source_fd)?;
                    }
                    close_after_success(self, fd)?;
                    return Ok(true);
                }
            }
            crate::parser::RedirectKind::Input | crate::parser::RedirectKind::ReadWrite => {
                if let Some(source) = target
                    .strip_prefix("<(")
                    .and_then(|target| target.strip_suffix(')'))
                {
                    if let Some(input) = self.process_substitution_output(source) {
                        let fd = self.allocate_dynamic_fd();
                        self.fd_table.open_input(
                            fd,
                            FdReadEndpoint::process_substitution(&input),
                            true,
                        );
                        self.set_fd_input_text(fd, input, true);
                        if redirect.kind == crate::parser::RedirectKind::ReadWrite {
                            self.set_fd_output_file(fd, target.clone(), true);
                        }
                        self.set_dynamic_fd_variable(name, fd);
                        close_after_success(self, fd)?;
                        return Ok(true);
                    }
                }

                let path = shell_path_to_windows(&target, &self.env_vars);
                let input = if is_null_device(&target) {
                    String::new()
                } else {
                    fs::read_to_string(&path)?
                };
                let fd = self.allocate_dynamic_fd();
                self.set_fd_input_text(fd, input, true);
                if redirect.kind == crate::parser::RedirectKind::ReadWrite {
                    self.set_fd_output_file(fd, target.clone(), true);
                }
                self.set_dynamic_fd_variable(name, fd);
                close_after_success(self, fd)?;
                return Ok(true);
            }
            crate::parser::RedirectKind::Output
            | crate::parser::RedirectKind::Append
            | crate::parser::RedirectKind::ClobberOutput => {
                let fd = self.allocate_dynamic_fd();
                if let Some(source) = target
                    .strip_prefix(">(")
                    .and_then(|target| target.strip_suffix(')'))
                {
                    if self.open_persistent_output_process_substitution(fd, &target)? {
                        self.set_dynamic_fd_variable(name, fd);
                        close_after_success(self, fd)?;
                        return Ok(true);
                    }
                    let _ = source;
                }
                if redirect.kind == crate::parser::RedirectKind::Append {
                    OpenOptions::new()
                        .create(true)
                        .append(true)
                        .open(shell_path_to_windows(&target, &self.env_vars))?;
                } else {
                    self.create_redirect_output(&target, redirect.clobber)?;
                }
                self.set_fd_output_file(fd, target, true);
                self.set_dynamic_fd_variable(name, fd);
                close_after_success(self, fd)?;
                return Ok(true);
            }
            _ => {}
        }

        Ok(true)
    }

    fn copy_persistent_input_fd(&mut self, target_fd: u32, source_fd: u32) {
        if self.fd_table.is_open_for_read(source_fd) {
            if self.fd_table.dup_input(target_fd, source_fd).is_ok() {
                match self
                    .fd_table
                    .entries
                    .get(&target_fd)
                    .and_then(|entry| entry.read.clone())
                {
                    Some(FdReadEndpoint::InheritedProcessStdin) => {
                        self.env_vars
                            .insert(fd_stdin_key(target_fd), FD_PROCESS_STDIN_TARGET.to_string());
                        self.env_vars.remove(&fd_stdin_offset_key(target_fd));
                        self.env_vars.remove(&fd_dynamic_input_key(target_fd));
                    }
                    Some(FdReadEndpoint::Text(_))
                    | Some(FdReadEndpoint::ProcessSubstitution(_)) => {
                        if let Some((input, offset)) = self.fd_table.input_snapshot(target_fd) {
                            self.env_vars.insert(fd_stdin_key(target_fd), input);
                            self.env_vars
                                .insert(fd_stdin_offset_key(target_fd), offset.to_string());
                            self.env_vars
                                .insert(fd_dynamic_input_key(target_fd), "1".to_string());
                        }
                    }
                    Some(FdReadEndpoint::File(path)) => {
                        self.env_vars.insert(
                            fd_stdin_key(target_fd),
                            shell_display_path(&path.to_string_lossy()),
                        );
                        self.env_vars.remove(&fd_stdin_offset_key(target_fd));
                        self.env_vars.remove(&fd_dynamic_input_key(target_fd));
                    }
                    Some(FdReadEndpoint::CoprocStdout(pid)) => {
                        self.env_vars.insert(
                            fd_stdin_key(target_fd),
                            format!("{FD_COPROC_STDIN_TARGET_PREFIX}{pid}"),
                        );
                        self.env_vars.remove(&fd_stdin_offset_key(target_fd));
                        self.env_vars.remove(&fd_dynamic_input_key(target_fd));
                    }
                    None => {}
                }
                self.env_vars.remove(&fd_closed_key(target_fd));
            } else {
                self.close_persistent_input_fd(target_fd);
                self.env_vars
                    .insert(fd_closed_key(target_fd), "1".to_string());
            }
            return;
        }
        // A source that is absent from FdTable is closed. Do not resurrect
        // shell input from the legacy environment mirror.
        self.close_persistent_input_fd(target_fd);
    }

    fn allocate_dynamic_fd(&mut self) -> u32 {
        self.fd_table.allocate_dynamic()
    }

    fn close_dynamic_fd(&mut self, name: &str) -> Result<(), ExecuteError> {
        if let Some(fd) = self.dynamic_fd_variable_value(name) {
            self.close_persistent_fd(fd)?;
        }
        Ok(())
    }

    fn close_dynamic_input_fd(&mut self, name: &str) {
        if let Some(fd) = self.dynamic_fd_variable_value(name) {
            self.close_persistent_input_fd(fd);
            if !self.fd_table.is_open_for_write(fd) {
                self.fd_table.close(fd);
                self.env_vars.insert(fd_closed_key(fd), "1".to_string());
            }
        }
    }

    fn close_dynamic_output_fd(&mut self, name: &str) -> Result<(), ExecuteError> {
        let Some(fd) = self.dynamic_fd_variable_value(name) else {
            return Ok(());
        };

        // A dynamic fd opened with `<>` is one shell descriptor.  Closing its
        // output side with `>&-` must release the descriptor completely;
        // otherwise its still-live input side prevents Bash's lowest-free fd
        // allocation from reusing the slot. Coprocess endpoints are modeled
        // as one-sided entries, so they retain the capability-specific path.
        if self.fd_table.read_endpoint(fd).is_some() {
            self.close_persistent_fd(fd)?;
            return Ok(());
        }

        // Coprocess input/output endpoints currently share the child PID as
        // their virtual descriptor. Close only the output capability so
        // `exec {COPROC[1]}>&-` does not invalidate `COPROC[0]` as well.
        self.close_persistent_output_fd(fd)?;
        if !self.fd_table.is_open_for_read(fd) {
            self.fd_table.close(fd);
            self.env_vars.insert(fd_closed_key(fd), "1".to_string());
        }
        Ok(())
    }

    fn dynamic_fd_variable_value(&self, name: &str) -> Option<u32> {
        if let Some((array_name, index)) = parse_array_numeric_subscript(name) {
            return self
                .array_element_parameter_value(&format!("{array_name}[{index}]"))
                .and_then(|value| value.parse::<u32>().ok());
        }

        let storage_name = self.resolved_variable_name(name)?;
        self.env_vars
            .get(&storage_name)
            .and_then(|value| value.parse::<u32>().ok())
    }

    fn set_dynamic_fd_variable(&mut self, name: &str, fd: u32) {
        if let Some((array_name, index)) = parse_array_numeric_subscript(name) {
            let storage_name = self
                .resolved_variable_name(array_name)
                .unwrap_or_else(|| array_name.to_string());
            let current = self
                .env_vars
                .get(&storage_name)
                .cloned()
                .unwrap_or_default();
            let mut entries = indexed_array_entries(&current);
            entries.insert(index, fd.to_string());
            self.env_vars
                .insert(storage_name.clone(), format_indexed_array_storage(entries));
            mark_env_name(&mut self.env_vars, ARRAY_VARS, &storage_name);
        } else {
            let storage_name = self
                .resolved_variable_name(name)
                .unwrap_or_else(|| name.to_string());
            self.env_vars.insert(storage_name, fd.to_string());
        }
    }

    pub(in crate::executor) fn execute_exec_command(
        &mut self,
        cmd: &CommandNode,
    ) -> Result<(), ExecuteError> {
        let status = self.execute_exec(cmd)?;
        let dynamic_fd_redirect = is_dynamic_fd_exec_redirect(cmd);
        self.exit_code = status;
        if !dynamic_fd_redirect && crate::builtins::exec::replaces_shell(&cmd.words[1..]) {
            return Err(ExecuteError::ExitCode(status));
        }
        Ok(())
    }

    fn exec_has_no_command_operand_after_expansion(&self, cmd: &CommandNode) -> bool {
        if cmd.words.len() <= 1 {
            return false;
        }

        let expanded_args: Vec<String> = cmd.words[1..]
            .iter()
            .map(|word| self.expand_word(word))
            .collect();
        !crate::builtins::exec::replaces_shell(&expanded_args)
            && (cmd.redirect_out.is_some()
                || cmd.append.is_some()
                || cmd.redirect_err.is_some()
                || cmd.redirect_err_append.is_some()
                || cmd.redirect_in.is_some())
    }
}

fn signal_trap_name(signal: i32) -> Option<String> {
    if signal <= 0 {
        return None;
    }
    crate::builtins::kill::translate_signal(&signal.to_string()).map(|name| format!("SIG{name}"))
}

fn is_dynamic_fd_exec_redirect(cmd: &CommandNode) -> bool {
    cmd.words.len() == 2
        && cmd
            .words
            .get(1)
            .and_then(|word| dynamic_fd_var_name(word))
            .is_some()
        && (cmd.redirect_in.is_some()
            || cmd.redirect_out.is_some()
            || cmd.append.is_some()
            || cmd.here_string.is_some()
            || cmd.heredoc.is_some())
}

fn exec_has_only_redirects(cmd: &CommandNode) -> bool {
    if cmd.words.len() == 1 {
        return true;
    }

    matches!(
        cmd.words.as_slice(),
        [command, fd_word]
            if command == "exec"
                && fd_word.chars().all(|ch| ch.is_ascii_digit())
                && cmd
                    .redirects
                    .iter()
                    .any(|redirect| redirect.fd.is_some_and(|fd| fd.to_string() == *fd_word))
    )
}

fn dynamic_fd_var_name(word: &str) -> Option<&str> {
    let name = word.strip_prefix('{')?.strip_suffix('}')?;
    if let Some((array_name, index)) = parse_array_subscript(name) {
        if is_shell_name(array_name) && index.parse::<usize>().is_ok() {
            return Some(name);
        }
    }
    let mut chars = name.chars();
    let first = chars.next()?;
    if !(first == '_' || first.is_ascii_alphabetic()) {
        return None;
    }
    chars
        .all(|ch| ch == '_' || ch.is_ascii_alphanumeric())
        .then_some(name)
}
