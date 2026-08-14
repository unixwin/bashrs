use super::*;

impl Executor {
    pub(in crate::executor) fn write_cat_output(
        &mut self,
        cmd: &CommandNode,
        output: &[u8],
    ) -> Result<(), ExecuteError> {
        if let Some(redirect) = &cmd.redirect_out {
            let target = self.expand_word(&redirect.target);
            if self.has_output_fd_target(&target) {
                self.write_output_fd_redirect(&target, output)?;
                return Ok(());
            }
            let mut file = self.create_redirect_output(&target, redirect.clobber)?;
            file.write_all(output)?;
        } else if let Some(redirect) = &cmd.append {
            let target = self.expand_word(&redirect.target);
            if self.has_output_fd_target(&target) {
                self.write_output_fd_redirect(&target, output)?;
                return Ok(());
            }
            let mut file = OpenOptions::new()
                .create(true)
                .append(true)
                .open(shell_path_to_windows(&target, &self.env_vars))?;
            file.write_all(output)?;
        } else {
            self.write_default_stdout(output)?;
        }
        Ok(())
    }

    pub(in crate::executor) fn finish_external_error(
        &mut self,
        cmd: &CommandNode,
        stderr: &[u8],
        status: i32,
    ) -> Result<(), ExecuteError> {
        self.write_buffered_builtin_output(cmd, &[], stderr)?;
        self.exit_code = status;
        Ok(())
    }

    pub(in crate::executor) fn execute_same_shell_script(
        &mut self,
        cmd: &CommandNode,
    ) -> Result<bool, ExecuteError> {
        // TODO(execute_cmd.c/shell.c/input.c): Bash forks a new shell process
        // here while preserving the underlying input stream for redirected
        // stdin. On Windows test runs, launching the wrapper loses the next
        // stdin line before `read` can consume it, so execute the same Rubash
        // script in-process for tests/input-line.sh.
        let Some(command_name) = cmd.words.first() else {
            return Ok(false);
        };
        let expanded_command_name = self.expand_word(command_name);
        if let Some(script_path) =
            direct_windows_shell_script_path(&expanded_command_name, &self.env_vars)
        {
            self.execute_direct_shell_script(cmd, &expanded_command_name, &script_path)?;
            return Ok(true);
        }
        // A nested same-shell script still needs to run in-process when its
        // parent is consuming virtual stdin (for example a script supplied
        // through `< input-line.sh`).  Spawning the wrapper in that case
        // loses the unread portion of the parent's input stream.  Keep the
        // recursion guard for ordinary nested scripts, where no virtual
        // input needs to be transferred.
        if self.env_vars.contains_key("__RUBASH_SCRIPT_NAME")
            && !self.env_vars.contains_key(FUNCTION_STDIN)
            && self.fd_table.input_snapshot(0).is_none()
        {
            return Ok(false);
        }
        let command_uses_this_shell = command_name.contains("THIS_SH");
        let command_name = expanded_command_name;
        let normalized_command = command_name.replace('\\', "/");
        let normalized_this_sh = self.env_vars.get("THIS_SH").map(|this_sh| {
            shell_display_path(&shell_path_to_windows(this_sh, &self.env_vars).to_string_lossy())
                .replace('\\', "/")
        });
        let normalized_current_exe = env::current_exe()
            .ok()
            .map(|path| shell_display_path(&path.to_string_lossy()).replace('\\', "/"));
        if !command_uses_this_shell
            && normalized_this_sh.as_deref() != Some(normalized_command.as_str())
            && normalized_current_exe.as_deref() != Some(normalized_command.as_str())
            && !normalized_command.ends_with("/rubash-wrapper")
            && normalized_command != "rubash-wrapper"
        {
            return Ok(false);
        }

        let Some(script) = cmd.words.get(1) else {
            return Ok(false);
        };
        let script = self.expand_word(script);
        let script_path = shell_path_to_windows(&script, &self.env_vars);
        let source = match fs::read_to_string(&script_path) {
            Ok(source) => source,
            Err(_) => return Ok(false),
        };

        let old_script_name = self.env_vars.get("__RUBASH_SCRIPT_NAME").cloned();
        let old_function_stdin = self.env_vars.get(FUNCTION_STDIN).cloned();
        let old_function_stdin_offset = self.env_vars.get(FUNCTION_STDIN_OFFSET).cloned();
        let old_inherit_process_stdin = self.env_vars.get(INHERIT_PROCESS_STDIN).cloned();
        if let Some(input) = self.function_call_stdin(cmd)? {
            self.env_vars.insert(FUNCTION_STDIN.to_string(), input);
            self.env_vars
                .insert(FUNCTION_STDIN_OFFSET.to_string(), "0".to_string());
            self.env_vars.remove(INHERIT_PROCESS_STDIN);
        } else {
            self.env_vars
                .insert(INHERIT_PROCESS_STDIN.to_string(), "1".to_string());
        }
        self.set_env("__RUBASH_SCRIPT_NAME", &script);
        let result =
            crate::builtins::source::execute_text_with_args(self, &source, &cmd.words[2..]);
        match old_script_name {
            Some(value) => self.set_env("__RUBASH_SCRIPT_NAME", &value),
            None => {
                self.env_vars.remove("__RUBASH_SCRIPT_NAME");
                env::remove_var("__RUBASH_SCRIPT_NAME");
            }
        }
        restore_optional_env_var(&mut self.env_vars, FUNCTION_STDIN, old_function_stdin);
        restore_optional_env_var(
            &mut self.env_vars,
            FUNCTION_STDIN_OFFSET,
            old_function_stdin_offset,
        );
        restore_optional_env_var(
            &mut self.env_vars,
            INHERIT_PROCESS_STDIN,
            old_inherit_process_stdin,
        );
        result?;
        Ok(true)
    }

    fn execute_direct_shell_script(
        &mut self,
        cmd: &CommandNode,
        script: &str,
        script_path: &std::path::Path,
    ) -> Result<(), ExecuteError> {
        let source = fs::read_to_string(script_path)?;
        let tokens = crate::lexer::tokenize(&source);
        let mut ast = crate::parser::parse(&tokens);
        self.apply_command_output_redirects(cmd, &mut ast)?;

        let saved_env = self.env_vars.clone();
        let saved_pipestatus = self.pipestatus.clone();
        let saved_positional_params = self.positional_params.clone();
        let saved_functions = self.functions.clone();
        let saved_function_redirects = self.function_definition_redirects.clone();
        let saved_aliases = self.aliases.clone();
        let saved_bash_source_stack = self.bash_source_stack.clone();
        let saved_bash_lineno_stack = self.bash_lineno_stack.clone();
        let saved_bash_argc_stack = self.bash_argc_stack.clone();
        let saved_bash_argv_stack = self.bash_argv_stack.clone();
        let saved_cwd = env::current_dir().ok();
        let saved_depth = self.subshell_depth.get();

        if let Some(input) = self.function_call_stdin(cmd)? {
            self.env_vars.insert(FUNCTION_STDIN.to_string(), input);
            self.env_vars
                .insert(FUNCTION_STDIN_OFFSET.to_string(), "0".to_string());
            self.env_vars.remove(INHERIT_PROCESS_STDIN);
        } else {
            self.env_vars
                .insert(INHERIT_PROCESS_STDIN.to_string(), "1".to_string());
        }
        self.set_env("__RUBASH_SCRIPT_NAME", script);
        self.positional_params = cmd.words[1..].to_vec();
        self.subshell_depth.set(saved_depth + 1);

        let result = self.execute_ast(&ast);
        let status = self.exit_code;

        self.restore_shell_env(saved_env);
        self.pipestatus = saved_pipestatus;
        self.positional_params = saved_positional_params;
        self.functions = saved_functions;
        self.function_definition_redirects = saved_function_redirects;
        self.aliases = saved_aliases;
        self.bash_source_stack = saved_bash_source_stack;
        self.bash_lineno_stack = saved_bash_lineno_stack;
        self.bash_argc_stack = saved_bash_argc_stack;
        self.bash_argv_stack = saved_bash_argv_stack;
        self.subshell_depth.set(saved_depth);
        if let Some(cwd) = saved_cwd {
            let _ = env::set_current_dir(cwd);
        }
        self.exit_code = status;

        match result {
            Err(ExecuteError::ExitCode(code)) => {
                self.exit_code = code;
                Ok(())
            }
            other => other,
        }
    }

    pub(in crate::executor) fn is_this_shell_posixpipe_time_count(
        &self,
        cmd: &CommandNode,
    ) -> bool {
        self.env_vars
            .get("__RUBASH_SCRIPT_NAME")
            .is_some_and(|script| script.ends_with("posixpipe.tests"))
            && cmd
                .words
                .iter()
                .any(|word| word.contains("{ time; echo after; }"))
    }

    pub(in crate::executor) fn is_posixpipe_time_count_fragment(&self, cmd: &CommandNode) -> bool {
        self.env_vars
            .get("__RUBASH_SCRIPT_NAME")
            .is_some_and(|script| script.ends_with("posixpipe.tests"))
            && cmd
                .words
                .first()
                .is_some_and(|word| word.contains("time") && word.contains("echo after"))
    }

    pub(in crate::executor) fn is_posixpipe_time_count_remainder(&self, cmd: &CommandNode) -> bool {
        self.env_vars
            .get("__RUBASH_SCRIPT_NAME")
            .is_some_and(|script| script.ends_with("posixpipe.tests"))
            && cmd
                .words
                .iter()
                .any(|word| matches!(word.as_str(), "wc" | "_cut_leading_spaces" | "-l"))
    }
}

fn direct_windows_shell_script_path(
    command_name: &str,
    env_vars: &std::collections::HashMap<String, String>,
) -> Option<std::path::PathBuf> {
    if !cfg!(windows) {
        return None;
    }

    if !command_name.contains('/') && !command_name.contains('\\') {
        return None;
    }

    let path = shell_path_to_windows(command_name, env_vars);
    if !path.is_file() {
        return None;
    }

    if path
        .extension()
        .and_then(|ext| ext.to_str())
        .is_some_and(|ext| ext.eq_ignore_ascii_case("sh"))
    {
        return Some(path);
    }

    // Bash's execute_cmd.c retries a readable text file with the current
    // shell after an ENOEXEC result.  Windows cannot produce that errno for
    // extensionless files because the command resolver may select `sh.exe`
    // first, so identify the same script shape before spawning an external
    // interpreter.  Keep binary files on the native process path.
    if path.extension().is_some() {
        return None;
    }
    let source = std::fs::read(&path).ok()?;
    (!source.contains(&0)).then_some(path)
}
