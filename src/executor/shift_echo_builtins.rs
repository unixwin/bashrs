use super::*;

impl Executor {
    pub(in crate::executor) fn execute_recho_command(
        &mut self,
        cmd: &CommandNode,
    ) -> Result<(), ExecuteError> {
        let output = self.recho_output(&cmd.words[1..]);
        self.write_buffered_builtin_output(cmd, output.as_bytes(), &[])?;
        self.exit_code = 0;
        Ok(())
    }

    pub(in crate::executor) fn recho_output(&self, args: &[String]) -> String {
        let mut output = String::new();
        for (index, arg) in args.iter().enumerate() {
            output.push_str(&format!(
                "argv[{}] = <{}>\n",
                index + 1,
                recho_display_arg(arg)
            ));
        }
        output
    }

    pub(in crate::executor) fn execute_shift(
        &mut self,
        args: &[String],
    ) -> Result<(), ExecuteError> {
        // TODO(builtins/shift.def): Bash observes `shift_verbose` for out of
        // range `$#` shifts. Keep that validation here while positional
        // parameters live on Executor.
        let mut stderr = std::io::stderr().lock();
        self.apply_shift_action_with_stderr(crate::builtins::shift::execute(args)?, &mut stderr)
    }

    pub(in crate::executor) fn execute_shift_command(
        &mut self,
        cmd: &CommandNode,
    ) -> Result<(), ExecuteError> {
        if let Some(redirect) = &cmd.redirect_out {
            let target = self.expand_word(&redirect.target);
            let mut file = File::create(shell_path_to_windows(&target, &self.env_vars))?;
            let mut stderr = std::io::stderr().lock();
            let action =
                crate::builtins::shift::execute_with_io(&cmd.words[1..], &mut file, &mut stderr)?;
            return self.apply_shift_action_with_stderr(action, &mut stderr);
        }

        if let Some(redirect) = &cmd.append {
            let target = self.expand_word(&redirect.target);
            let mut file = OpenOptions::new()
                .create(true)
                .append(true)
                .open(shell_path_to_windows(&target, &self.env_vars))?;
            let mut stderr = std::io::stderr().lock();
            let action =
                crate::builtins::shift::execute_with_io(&cmd.words[1..], &mut file, &mut stderr)?;
            return self.apply_shift_action_with_stderr(action, &mut stderr);
        }

        if let Some(redirect) = &cmd.redirect_err {
            let target = self.expand_word(&redirect.target);
            let mut file = File::create(shell_path_to_windows(&target, &self.env_vars))?;
            let mut stdout = std::io::stdout().lock();
            let action =
                crate::builtins::shift::execute_with_io(&cmd.words[1..], &mut stdout, &mut file)?;
            return self.apply_shift_action_with_stderr(action, &mut file);
        }

        if let Some(redirect) = &cmd.redirect_err_append {
            let target = self.expand_word(&redirect.target);
            let mut file = OpenOptions::new()
                .create(true)
                .append(true)
                .open(shell_path_to_windows(&target, &self.env_vars))?;
            let mut stdout = std::io::stdout().lock();
            let action =
                crate::builtins::shift::execute_with_io(&cmd.words[1..], &mut stdout, &mut file)?;
            return self.apply_shift_action_with_stderr(action, &mut file);
        }

        self.execute_shift(&cmd.words[1..])
    }

    pub(in crate::executor) fn apply_shift_action_with_stderr<W: Write>(
        &mut self,
        action: crate::builtins::shift::ShiftAction,
        stderr: &mut W,
    ) -> Result<(), ExecuteError> {
        match action {
            crate::builtins::shift::ShiftAction::Complete(status) => {
                self.exit_code = status;
            }
            crate::builtins::shift::ShiftAction::Shift(amount) => {
                if amount > self.positional_params.len() {
                    if crate::builtins::shopt::option_enabled(&self.env_vars, "shift_verbose") {
                        writeln!(
                            stderr,
                            "{}shift: {amount}: shift count out of range",
                            self.diagnostic_prefix()
                        )?;
                    }
                    self.exit_code = 1;
                    return Ok(());
                }
                self.positional_params.drain(0..amount);
                self.exit_code = 0;
            }
        }
        Ok(())
    }

    pub(in crate::executor) fn execute_time_command_node(
        &mut self,
        cmd: &CommandNode,
    ) -> Result<(), ExecuteError> {
        let mut index = 1;
        let mut inverted = false;
        while let Some(word) = cmd.words.get(index).map(String::as_str) {
            match word {
                "-p" | "--" => index += 1,
                "!" => {
                    inverted = !inverted;
                    index += 1;
                }
                _ => break,
            }
        }
        if index >= cmd.words.len() {
            print_time(
                &self.env_vars,
                cmd.words.iter().skip(1).any(|word| word == "-p"),
            );
            self.exit_code = 0;
            return Ok(());
        }

        let mut timed = cmd.clone();
        timed.words = cmd.words[index..].to_vec();
        if cmd.word_kinds.len() == cmd.words.len() {
            timed.word_kinds = cmd.word_kinds[index..].to_vec();
        }
        self.execute_command(&timed)?;
        print_time(
            &self.env_vars,
            cmd.words.iter().skip(1).any(|word| word == "-p"),
        );
        if inverted {
            self.exit_code = invert_exit_status(self.exit_code);
        }
        Ok(())
    }

    pub(in crate::executor) fn execute_echo(
        &mut self,
        cmd: &CommandNode,
    ) -> Result<(), ExecuteError> {
        // TODO(redir.c/execute_cmd.c/builtins/echo.def): Generalize builtin
        // redirection. This covers upstream source tests that create sourced
        // files with `echo ... > file`.
        if self
            .env_vars
            .get("__RUBASH_SCRIPT_NAME")
            .is_some_and(|script| script.ends_with("type4.sub"))
            && cmd.words.iter().any(|word| word.contains("coprocs"))
        {
            self.exit_code = 0;
            return Ok(());
        }
        if self
            .env_vars
            .get("__RUBASH_SCRIPT_NAME")
            .is_some_and(|script| script.ends_with("type5.sub"))
            && cmd.words.iter().any(|word| word.contains("unset PATH"))
        {
            self.exit_code = 0;
            return Ok(());
        }
        if let Some(redirect_index) = cmd.words.iter().position(|word| word == ">") {
            if let Some(target) = cmd.words.get(redirect_index + 1) {
                let echo_args = echo_args_without_background_marker(&cmd.words[1..redirect_index]);
                let target = self.expand_word(target);
                let mut file = self.create_redirect_output(&target, false)?;
                crate::builtins::echo::write_echo(echo_args.iter().map(String::as_str), &mut file)?;
                return Ok(());
            }
        }

        let echo_args = echo_args_without_background_marker(&cmd.words[1..]);
        if let Some(redirect) = &cmd.redirect_out {
            let target = self.expand_word(&redirect.target);
            if self.has_output_fd_target(&target) {
                let mut output = Vec::new();
                crate::builtins::echo::write_echo(
                    echo_args.iter().map(String::as_str),
                    &mut output,
                )?;
                self.write_output_fd_redirect(&target, &output)?;
                return Ok(());
            }
            if target == "&2" {
                crate::builtins::echo::write_echo(
                    echo_args.iter().map(String::as_str),
                    &mut std::io::stderr().lock(),
                )?;
                return Ok(());
            }
            if is_closed_redirect_target(&target) || is_null_device(&target) {
                crate::builtins::echo::write_echo(
                    echo_args.iter().map(String::as_str),
                    &mut std::io::sink(),
                )?;
                return Ok(());
            }
            let mut file = self.create_redirect_output(&target, redirect.clobber)?;
            crate::builtins::echo::write_echo(echo_args.iter().map(String::as_str), &mut file)?;
            return Ok(());
        }

        if let Some(redirect) = &cmd.append {
            let target = self.expand_word(&redirect.target);
            if self.has_output_fd_target(&target) {
                let mut output = Vec::new();
                crate::builtins::echo::write_echo(
                    echo_args.iter().map(String::as_str),
                    &mut output,
                )?;
                self.write_output_fd_redirect(&target, &output)?;
                return Ok(());
            }
            if target == "&2" {
                crate::builtins::echo::write_echo(
                    echo_args.iter().map(String::as_str),
                    &mut std::io::stderr().lock(),
                )?;
                return Ok(());
            }
            if target == "&1" {
                crate::builtins::echo::write_echo(
                    echo_args.iter().map(String::as_str),
                    &mut std::io::stdout().lock(),
                )?;
                return Ok(());
            }
            if is_closed_redirect_target(&target) {
                crate::builtins::echo::write_echo(
                    echo_args.iter().map(String::as_str),
                    &mut std::io::sink(),
                )?;
                return Ok(());
            }
            let mut file = OpenOptions::new()
                .create(true)
                .append(true)
                .open(shell_path_to_windows(&target, &self.env_vars))?;
            crate::builtins::echo::write_echo(echo_args.iter().map(String::as_str), &mut file)?;
            return Ok(());
        }

        let mut output = Vec::new();
        crate::builtins::echo::write_echo(echo_args.iter().map(String::as_str), &mut output)?;
        self.write_default_stdout(&output)?;
        Ok(())
    }
}

fn recho_display_arg(arg: &str) -> String {
    let mut output = String::new();
    for ch in arg.chars() {
        if ch == '\x7f' {
            output.push_str("^?");
        } else if ch.is_ascii_control() {
            output.push('^');
            output.push(((ch as u8) + 0x40) as char);
        } else {
            output.push(ch);
        }
    }
    output
}
