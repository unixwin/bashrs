use super::*;
use std::io::IsTerminal;

impl Executor {
    pub(in crate::executor) fn handle_external_file_builtins(
        &mut self,
        cmd: &CommandNode,
    ) -> Result<bool, ExecuteError> {
        if !self.external_file_builtins_enabled {
            return Ok(false);
        }
        match cmd.words[0].as_str() {
            "/bin/pwd" | "/usr/bin/pwd" => {
                let mut pwd_cmd = cmd.clone();
                pwd_cmd.words[0] = "pwd".to_string();
                self.exit_code = self.execute_pwd(&pwd_cmd)?;
                Ok(true)
            }
            "mkdir" => self.external_mkdir(cmd),
            "touch" => self.external_touch(cmd),
            "chmod" => {
                self.exit_code = 0;
                Ok(true)
            }
            "cp" => self.external_cp(cmd),
            "rm" => self.external_rm(cmd),
            "rmdir" => self.external_rmdir(cmd),
            "cat" | "/bin/cat" | "/usr/bin/cat" => self.external_cat(cmd),
            "sed" => self.external_sed(cmd),
            "mkfifo" => self.external_mkfifo(cmd),
            "tty" | "/bin/tty" | "/usr/bin/tty" => self.external_tty(cmd),
            _ => Ok(false),
        }
    }

    fn external_tty(&mut self, cmd: &CommandNode) -> Result<bool, ExecuteError> {
        let silent = cmd
            .words
            .iter()
            .skip(1)
            .any(|arg| arg == "-s" || arg == "--silent" || arg == "--quiet");
        let output = if std::io::stdin().is_terminal() {
            // A real tty device name is platform-specific; the non-tty case is
            // the compatibility-critical path for bashdb command input.
            "/dev/tty
"
        } else {
            "not a tty
"
        };
        if !silent {
            self.write_cat_output(cmd, output.as_bytes())?;
        }
        self.exit_code = if std::io::stdin().is_terminal() { 0 } else { 1 };
        Ok(true)
    }

    fn external_mkdir(&mut self, cmd: &CommandNode) -> Result<bool, ExecuteError> {
        for path in &cmd.words[1..] {
            fs::create_dir_all(shell_path_to_windows(
                &self.expand_word(path),
                &self.env_vars,
            ))?;
        }
        self.exit_code = 0;
        Ok(true)
    }

    fn external_touch(&mut self, cmd: &CommandNode) -> Result<bool, ExecuteError> {
        for path in &cmd.words[1..] {
            let expanded = self.expand_word(path);
            let target = shell_path_to_windows(&expanded, &self.env_vars);
            if let Err(error) = File::create(target) {
                if !(cfg!(windows) && contains_windows_forbidden_posix_filename_char(&expanded)) {
                    return Err(error.into());
                }
            }
        }
        self.exit_code = 0;
        Ok(true)
    }

    fn external_cp(&mut self, cmd: &CommandNode) -> Result<bool, ExecuteError> {
        let mut args = Vec::new();
        for word in &cmd.words[1..] {
            if !word.starts_with('-') {
                args.push(self.expand_word(word));
            }
        }

        if args.len() < 2 {
            eprintln!("{}cp: missing file operand", self.diagnostic_prefix());
            self.exit_code = 1;
            return Ok(true);
        }

        let destination =
            shell_path_to_windows(args.last().expect("cp destination"), &self.env_vars);
        if args.len() > 2 && !destination.is_dir() {
            eprintln!(
                "{}cp: target '{}' is not a directory",
                self.diagnostic_prefix(),
                args.last().expect("cp destination")
            );
            self.exit_code = 1;
            return Ok(true);
        }

        for source in &args[..args.len() - 1] {
            let source_path = shell_path_to_windows(source, &self.env_vars);
            let target_path = if destination.is_dir() {
                let Some(name) = source_path.file_name() else {
                    eprintln!(
                        "{}cp: cannot stat '{}': No such file or directory",
                        self.diagnostic_prefix(),
                        source
                    );
                    self.exit_code = 1;
                    return Ok(true);
                };
                destination.join(name)
            } else {
                destination.clone()
            };

            if let Err(error) = fs::copy(&source_path, &target_path) {
                // GNU cp wording: source stat failures use "cannot stat",
                // everything else reports the destination operation.
                if !source_path.exists() {
                    eprintln!(
                        "{}cp: cannot stat '{}': {}",
                        self.diagnostic_prefix(),
                        source,
                        crate::posix_errors::message(&error)
                    );
                } else {
                    eprintln!(
                        "{}cp: cannot create '{}': {}",
                        self.diagnostic_prefix(),
                        target_path.display(),
                        crate::posix_errors::message(&error)
                    );
                }
                self.exit_code = 1;
                return Ok(true);
            }
        }

        self.exit_code = 0;
        Ok(true)
    }

    fn external_rm(&mut self, cmd: &CommandNode) -> Result<bool, ExecuteError> {
        let force = cmd
            .words
            .iter()
            .skip(1)
            .any(|arg| arg.starts_with('-') && arg.contains('f'));
        let mut status = 0;
        let mut stderr = Vec::new();
        for path in cmd.words.iter().skip(1).filter(|arg| !arg.starts_with('-')) {
            let expanded = self.expand_word(path);
            let target = shell_path_to_windows(&expanded, &self.env_vars);
            let result = if target.is_dir() {
                fs::remove_dir_all(&target)
            } else {
                fs::remove_file(&target)
            };
            if let Err(error) = result {
                if !force {
                    status = 1;
                    let message = if matches!(
                        error.kind(),
                        io::ErrorKind::NotFound | io::ErrorKind::InvalidInput
                    ) || (cfg!(windows)
                        && contains_windows_forbidden_posix_filename_char(&expanded))
                    {
                        "No such file or directory".to_string()
                    } else {
                        crate::posix_errors::message(&error)
                    };
                    writeln!(&mut stderr, "rm: cannot remove '{}': {message}", expanded)?;
                }
            }
        }
        if !stderr.is_empty() {
            self.write_buffered_builtin_output(cmd, &[], &stderr)?;
        }
        self.exit_code = status;
        Ok(true)
    }

    fn external_rmdir(&mut self, cmd: &CommandNode) -> Result<bool, ExecuteError> {
        for path in &cmd.words[1..] {
            let _ = fs::remove_dir(shell_path_to_windows(
                &self.expand_word(path),
                &self.env_vars,
            ));
        }
        self.exit_code = 0;
        Ok(true)
    }

    fn external_cat(&mut self, cmd: &CommandNode) -> Result<bool, ExecuteError> {
        if let Some(redirect) = &cmd.redirect_in {
            if redirect.fd.unwrap_or(0) == 0 {
                let target = self.expand_word(&redirect.target);
                if let Some(fd) = redirect_target_fd(&target) {
                    if let Some(FdReadEndpoint::CoprocStdout(pid)) = self.fd_table.read_endpoint(fd)
                    {
                        if let Some(mut reader) = self.coproc_stdout_readers.remove(&pid) {
                            use std::io::Read;
                            let mut input = Vec::new();
                            reader.read_to_end(&mut input)?;
                            self.fd_table.close_input(fd);
                            self.write_cat_output(cmd, &input)?;
                            self.exit_code = 0;
                            return Ok(true);
                        }
                    }
                }
            }
        }

        if cmd.heredoc.is_some() {
            let input = self.stdin_string_for_command_mut(cmd).unwrap_or_default();
            if let Some(redirect) = &cmd.append {
                let target = self.expand_word(&redirect.target);
                let mut file = OpenOptions::new()
                    .create(true)
                    .append(true)
                    .open(shell_path_to_windows(&target, &self.env_vars))?;
                file.write_all(input.as_bytes())?;
                self.exit_code = 0;
                return Ok(true);
            }

            if let Some(redirect) = &cmd.redirect_out {
                let target = self.expand_word(&redirect.target);
                let mut file = self.create_redirect_output(&target, redirect.clobber)?;
                file.write_all(input.as_bytes())?;
                self.exit_code = 0;
                return Ok(true);
            }
        }

        if let Some(input) = self.stdin_string_for_command_mut(cmd) {
            self.write_cat_output(cmd, input.as_bytes())?;
            self.exit_code = 0;
            return Ok(true);
        }

        if !cat_has_file_operands(cmd) {
            if let Some(input) = self.read_function_stdin('\0', None, false) {
                self.write_cat_output(cmd, input.as_bytes())?;
                self.exit_code = 0;
                return Ok(true);
            }
            if cmd.redirect_in.is_none()
                && cmd.heredoc.is_none()
                && cmd.here_string.is_none()
                && self.env_vars.get(INHERIT_PROCESS_STDIN).map(String::as_str) == Some("1")
            {
                return self.stream_inherited_cat(cmd);
            }
            if cmd.words.len() <= 1 {
                return Ok(false);
            }
            return Ok(false);
        }

        let mut output = Vec::new();
        for word in cat_file_operands(cmd) {
            let target = self.expand_word(word);
            match fs::read(shell_path_to_windows(&target, &self.env_vars)) {
                Ok(bytes) => output.extend(bytes),
                Err(_) => {
                    let mut stderr = Vec::new();
                    writeln!(
                        &mut stderr,
                        "{}cat: {}: No such file or directory",
                        self.diagnostic_prefix(),
                        target
                    )?;
                    self.write_buffered_builtin_output(cmd, &[], &stderr)?;
                    self.exit_code = 1;
                    return Ok(true);
                }
            }
        }
        self.write_cat_output(cmd, &output)?;
        self.exit_code = 0;
        Ok(true)
    }

    fn stream_inherited_cat(&mut self, cmd: &CommandNode) -> Result<bool, ExecuteError> {
        use std::io::Read;

        let mut stdin = std::io::stdin().lock();
        let mut buffer = [0_u8; 8192];
        loop {
            let count = stdin.read(&mut buffer)?;
            if count == 0 {
                break;
            }
            self.write_cat_output(cmd, &buffer[..count])?;
        }
        self.exit_code = 0;
        Ok(true)
    }

    fn external_sed(&mut self, cmd: &CommandNode) -> Result<bool, ExecuteError> {
        let args = cmd.words[1..]
            .iter()
            .map(|word| self.expand_word(word))
            .collect::<Vec<_>>();
        if apply_simple_sed_args("", &args).is_none() {
            return Ok(false);
        }
        let Some(input) = self
            .stdin_string_for_command_mut(cmd)
            .or_else(|| self.read_function_stdin('\0', None, false))
            .or_else(|| self.read_inherited_process_stdin_to_string())
        else {
            return Ok(false);
        };
        let Some(output) = apply_simple_sed_args(&input, &args) else {
            return Ok(false);
        };
        self.write_cat_output(cmd, output.as_bytes())?;
        self.exit_code = 0;
        Ok(true)
    }

    fn external_mkfifo(&mut self, cmd: &CommandNode) -> Result<bool, ExecuteError> {
        for path in &cmd.words[1..] {
            let target = shell_path_to_windows(&self.expand_word(path), &self.env_vars);
            let _ = File::create(target)?;
        }
        self.exit_code = 0;
        Ok(true)
    }
}

fn cat_file_operands(cmd: &CommandNode) -> Vec<&String> {
    let mut operands = Vec::new();
    let mut skip_next = false;
    let redirect_targets = cat_redirect_targets(cmd);

    for word in cmd.words.iter().skip(1) {
        if skip_next {
            skip_next = false;
            continue;
        }
        if is_cat_redirect_operator_word(word) {
            skip_next = true;
            continue;
        }
        if word.starts_with('-') {
            continue;
        }
        if redirect_targets.iter().any(|target| *target == word) {
            continue;
        }
        operands.push(word);
    }

    operands
}

fn cat_redirect_targets(cmd: &CommandNode) -> Vec<&String> {
    [
        cmd.redirect_in.as_ref(),
        cmd.redirect_out.as_ref(),
        cmd.append.as_ref(),
        cmd.redirect_err.as_ref(),
        cmd.redirect_err_append.as_ref(),
    ]
    .into_iter()
    .flatten()
    .map(|redirect| &redirect.target)
    .collect()
}

fn cat_has_file_operands(cmd: &CommandNode) -> bool {
    !cat_file_operands(cmd).is_empty()
}

fn is_cat_redirect_operator_word(word: &str) -> bool {
    matches!(
        word,
        "<" | ">" | ">|" | ">>" | "2>" | "2>|" | "2>>" | "&>" | "&>>"
    ) || word.chars().next().is_some_and(|ch| ch.is_ascii_digit())
        && matches!(
            word.chars()
                .skip_while(|ch| ch.is_ascii_digit())
                .collect::<String>()
                .as_str(),
            "<" | ">" | ">|" | ">>"
        )
}
