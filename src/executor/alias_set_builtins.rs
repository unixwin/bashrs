use super::*;

impl Executor {
    pub(in crate::executor) fn execute_unalias(
        &mut self,
        cmd: &CommandNode,
    ) -> Result<i32, ExecuteError> {
        // TODO(redir.c/execute_cmd.c): Bash applies redirections around
        // builtins using unwind-protected fd mutation. This only handles
        // stderr redirection for upstream alias tests.
        if let Some(redirect) = &cmd.redirect_err {
            let target = self.expand_word(&redirect.target);
            if is_null_device(&target) {
                return Ok(crate::builtins::alias::unalias_with_io(
                    &cmd.words[1..],
                    &mut self.aliases,
                    &mut std::io::sink(),
                )?);
            }

            let mut file = self.create_redirect_output(&target, redirect.clobber)?;
            return Ok(crate::builtins::alias::unalias_with_io(
                &cmd.words[1..],
                &mut self.aliases,
                &mut file,
            )?);
        }

        if let Some(redirect) = &cmd.redirect_err_append {
            let target = self.expand_word(&redirect.target);
            let mut file = OpenOptions::new()
                .create(true)
                .append(true)
                .open(shell_path_to_windows(&target, &self.env_vars))?;
            return Ok(crate::builtins::alias::unalias_with_io(
                &cmd.words[1..],
                &mut self.aliases,
                &mut file,
            )?);
        }

        Ok(crate::builtins::alias::unalias(
            &cmd.words[1..],
            &mut self.aliases,
        )?)
    }

    pub(in crate::executor) fn execute_alias(
        &mut self,
        cmd: &CommandNode,
    ) -> Result<i32, ExecuteError> {
        if let Some(redirect) = &cmd.redirect_out {
            let target = self.expand_word(&redirect.target);
            let mut file = File::create(shell_path_to_windows(&target, &self.env_vars))?;
            return Ok(crate::builtins::alias::alias_with_io(
                &cmd.words[1..],
                &mut self.aliases,
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
            return Ok(crate::builtins::alias::alias_with_io(
                &cmd.words[1..],
                &mut self.aliases,
                &mut file,
                &mut std::io::stderr().lock(),
            )?);
        }

        if let Some(redirect) = &cmd.redirect_err {
            let target = self.expand_word(&redirect.target);
            if is_null_device(&target) {
                return Ok(crate::builtins::alias::alias_with_io(
                    &cmd.words[1..],
                    &mut self.aliases,
                    &mut std::io::stdout(),
                    &mut std::io::sink(),
                )?);
            }
            let mut file = File::create(shell_path_to_windows(&target, &self.env_vars))?;
            return Ok(crate::builtins::alias::alias_with_io(
                &cmd.words[1..],
                &mut self.aliases,
                &mut std::io::stdout(),
                &mut file,
            )?);
        }

        if let Some(redirect) = &cmd.redirect_err_append {
            let target = self.expand_word(&redirect.target);
            let mut file = OpenOptions::new()
                .create(true)
                .append(true)
                .open(shell_path_to_windows(&target, &self.env_vars))?;
            return Ok(crate::builtins::alias::alias_with_io(
                &cmd.words[1..],
                &mut self.aliases,
                &mut std::io::stdout(),
                &mut file,
            )?);
        }

        let status = crate::builtins::alias::alias(&cmd.words[1..], &mut self.aliases)?;
        self.record_alias_definition_lines(&cmd.words[1..], cmd.line);
        Ok(status)
    }

    fn record_alias_definition_lines(&mut self, args: &[String], line: Option<usize>) {
        let Some(line) = line else {
            return;
        };
        let mut index = 0;
        while let Some(arg) = args.get(index) {
            if arg == "--" {
                index += 1;
                continue;
            }
            if arg.starts_with('-') && arg != "-" {
                index += 1;
                continue;
            }
            if let Some((name, _)) = arg.split_once('=') {
                if !name.is_empty() {
                    self.env_vars
                        .insert(format!("__RUBASH_ALIAS_LINE_{name}"), line.to_string());
                }
            }
            index += 1;
        }
    }

    pub(in crate::executor) fn execute_set(
        &mut self,
        cmd: &CommandNode,
    ) -> Result<i32, ExecuteError> {
        if let Some(redirect) = &cmd.redirect_out {
            let target = self.expand_word(&redirect.target);
            let mut file = File::create(shell_path_to_windows(&target, &self.env_vars))?;
            return Ok(crate::builtins::set::set_with_io(
                cmd.words[1..].iter().map(String::as_str),
                &mut self.env_vars,
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
            return Ok(crate::builtins::set::set_with_io(
                cmd.words[1..].iter().map(String::as_str),
                &mut self.env_vars,
                &mut file,
                &mut std::io::stderr().lock(),
            )?);
        }

        if let Some(redirect) = &cmd.redirect_err {
            let target = self.expand_word(&redirect.target);
            if is_null_device(&target) {
                return Ok(crate::builtins::set::set_with_io(
                    cmd.words[1..].iter().map(String::as_str),
                    &mut self.env_vars,
                    &mut crate::executor::shell_options::GlobalStdout,
                    &mut std::io::sink(),
                )?);
            }
            let mut file = File::create(shell_path_to_windows(&target, &self.env_vars))?;
            return Ok(crate::builtins::set::set_with_io(
                cmd.words[1..].iter().map(String::as_str),
                &mut self.env_vars,
                &mut crate::executor::shell_options::GlobalStdout,
                &mut file,
            )?);
        }

        if let Some(redirect) = &cmd.redirect_err_append {
            let target = self.expand_word(&redirect.target);
            let mut file = OpenOptions::new()
                .create(true)
                .append(true)
                .open(shell_path_to_windows(&target, &self.env_vars))?;
            return Ok(crate::builtins::set::set_with_io(
                cmd.words[1..].iter().map(String::as_str),
                &mut self.env_vars,
                &mut crate::executor::shell_options::GlobalStdout,
                &mut file,
            )?);
        }

        Ok(crate::builtins::set::set(
            &cmd.words[1..],
            &mut self.env_vars,
        )?)
    }

    pub(in crate::executor) fn execute_set_command(
        &mut self,
        cmd: &CommandNode,
    ) -> Result<(), ExecuteError> {
        if self.apply_simple_set_flags(&cmd.words[1..]) {
            self.exit_code = 0;
            return Ok(());
        }
        if self.apply_set_positional_operands(&cmd.words[1..]) {
            self.exit_code = 0;
            return Ok(());
        }
        if cmd.words.get(1).map(String::as_str) == Some("--") {
            // TODO(builtins/set.def/variables.c): `set --` replaces shell
            // positional parameters. Full set option parsing lives in
            // builtins::set; this branch covers upstream source tests that
            // inspect `$@`.
            self.positional_params = cmd.words[2..].to_vec();
            self.exit_code = 0;
            return Ok(());
        }
        self.exit_code = self.execute_set(cmd)?;
        Ok(())
    }
}
