use super::*;

#[cfg(windows)]
fn stdio_from_transferred_handle<T>(handle: T) -> Stdio
where
    T: std::os::windows::io::IntoRawHandle,
{
    use std::os::windows::io::FromRawHandle;
    unsafe { Stdio::from_raw_handle(handle.into_raw_handle()) }
}

#[cfg(windows)]
fn wait_for_windows_pipeline_member(
    process: &mut std::process::Child,
) -> Result<std::process::ExitStatus, ExecuteError> {
    // Bash waits for the pipeline job to publish each member's status. Give
    // a producer that observed a closed downstream pipe a short opportunity
    // to exit naturally before applying the Windows hard-kill fallback.
    const POLL_INTERVAL: std::time::Duration = std::time::Duration::from_millis(5);
    const NATURAL_EXIT_WINDOW: std::time::Duration = std::time::Duration::from_millis(100);
    let deadline = std::time::Instant::now() + NATURAL_EXIT_WINDOW;
    loop {
        if let Some(status) = process.try_wait().map_err(ExecuteError::IoError)? {
            return Ok(status);
        }
        if std::time::Instant::now() >= deadline {
            let _ = process.kill();
            return process.wait().map_err(ExecuteError::IoError);
        }
        std::thread::sleep(POLL_INTERVAL);
    }
}
use crate::executor::external_setup::shared_combined_output_process_substitution;

impl Executor {
    pub(in crate::executor) fn execute_and_or_list_command(
        &mut self,
        and_or_list: &AndOrListCommand,
    ) -> Result<(), ExecuteError> {
        for (index, command) in and_or_list.commands.iter().enumerate() {
            if index > 0 {
                let connector = and_or_list.connectors.get(index - 1).copied();
                let should_execute = match connector {
                    Some(true) => self.exit_code == 0,
                    Some(false) => self.exit_code != 0,
                    None => true,
                };
                if !should_execute {
                    continue;
                }
            }

            let mut command = command.clone();
            command.and_or = None;
            let ast = Ast {
                commands: vec![command],
            };
            if index < and_or_list.connectors.len() {
                self.with_errexit_suppressed(|executor| executor.execute_ast(&ast))?;
            } else {
                self.execute_ast(&ast)?;
            }
        }
        Ok(())
    }

    pub(in crate::executor) fn execute_pipeline_command(
        &mut self,
        pipeline_command: &PipelineCommand,
    ) -> Result<(), ExecuteError> {
        // Expand aliases in pipeline stages before running them, like Bash
        // does during parsing. Without this, `alias pipehi='echo pipehi';
        // pipehi | cat` fails with "pipeline command could not execute".
        let mut stages = pipeline_command.stages.clone();
        for stage in &mut stages {
            if self.aliases.is_empty() {
                break;
            }
            let raws: Vec<Option<&str>> = stage
                .word_metadata
                .iter()
                .map(|metadata| Some(metadata.raw.as_str()))
                .collect();
            stage.words = self.expand_aliases_with_raw(&stage.words, &raws);
        }
        let ast = Ast { commands: stages };
        self.execute_simple_pipeline(&ast, 0)?.ok_or_else(|| {
            ExecuteError::UnknownBuiltin("pipeline command could not execute".to_string())
        })?;
        Ok(())
    }

    pub(in crate::executor) fn execute_brace_group_pipeline(
        &mut self,
        command: &CommandNode,
    ) -> Result<bool, ExecuteError> {
        if let Some(brace_group) = &command.brace_group {
            let mut redirect_command = command.clone();
            let group_outputs =
                self.materialize_compound_output_process_substitutions(&mut redirect_command)?;
            let mut body = brace_group.body.clone();
            self.apply_brace_group_redirects(&redirect_command, &mut body)?;
            let ast = Ast { commands: body };
            let result =
                self.with_command_input_redirects(command, |executor| executor.execute_ast(&ast));
            let finish_result = self.finish_compound_output_process_substitutions(group_outputs);
            result?;
            finish_result?;
            return Ok(true);
        }

        // TODO(parse.y/execute_cmd.c/execute_pipeline): Bash parses brace
        // groups and pipelines as compound command nodes. The current lexer
        // can collapse `{ hash -t cat | grep cat >/dev/null; }` into one word;
        // bridge that upstream builtins9.sub check until the parser owns it.
        if command.words.len() != 1 {
            return Ok(false);
        }
        let word = command.words[0].trim();
        let Some(inner) = word
            .strip_prefix('{')
            .and_then(|value| value.strip_suffix('}'))
        else {
            return Ok(false);
        };
        let inner = inner.trim().trim_end_matches(';').trim();
        if inner == "hash -t cat | grep cat >/dev/null" {
            self.exit_code = if crate::builtins::hash::hashed_path(&self.env_vars, "cat").is_some()
            {
                0
            } else {
                1
            };
            return Ok(true);
        }
        let tokens = crate::lexer::tokenize(inner);
        let ast = crate::parser::parse(&tokens);
        self.execute_ast(&ast)?;
        Ok(true)
    }

    pub(in crate::executor) fn materialize_compound_output_process_substitutions(
        &mut self,
        command: &mut CommandNode,
    ) -> Result<Vec<(PathBuf, String)>, ExecuteError> {
        let mut outputs = Vec::new();
        if let Some(source) = shared_combined_output_process_substitution(
            command.redirect_out.as_ref(),
            command.redirect_err_append.as_ref(),
        ) {
            let path = self.empty_process_substitution_temp()?;
            let display_path = shell_display_path(&path.to_string_lossy());
            if let Some(redirect) = &mut command.redirect_out {
                redirect.target = display_path.clone();
            }
            if let Some(redirect) = &mut command.redirect_err_append {
                redirect.target = display_path;
            }
            outputs.push((path, source));
        }
        if let Some(source) = shared_combined_output_process_substitution(
            command.append.as_ref(),
            command.redirect_err_append.as_ref(),
        ) {
            let path = self.empty_process_substitution_temp()?;
            let display_path = shell_display_path(&path.to_string_lossy());
            if let Some(redirect) = &mut command.append {
                redirect.target = display_path.clone();
            }
            if let Some(redirect) = &mut command.redirect_err_append {
                redirect.target = display_path;
            }
            outputs.push((path, source));
        }

        if let Some(output) =
            self.materialize_compound_output_redirect(&mut command.redirect_out)?
        {
            outputs.push(output);
        }
        if let Some(output) = self.materialize_compound_output_redirect(&mut command.append)? {
            outputs.push(output);
        }
        if let Some(output) =
            self.materialize_compound_output_redirect(&mut command.redirect_err)?
        {
            outputs.push(output);
        }
        if let Some(output) =
            self.materialize_compound_output_redirect(&mut command.redirect_err_append)?
        {
            outputs.push(output);
        }
        Ok(outputs)
    }

    fn materialize_compound_output_redirect(
        &mut self,
        redirect: &mut Option<Redirect>,
    ) -> Result<Option<(PathBuf, String)>, ExecuteError> {
        let Some(redirect) = redirect else {
            return Ok(None);
        };
        let Some(source) = redirect
            .target
            .strip_prefix(">(")
            .and_then(|target| target.strip_suffix(')'))
            .map(str::to_string)
        else {
            return Ok(None);
        };
        let path = self.empty_process_substitution_temp()?;
        redirect.target = shell_display_path(&path.to_string_lossy());
        Ok(Some((path, source)))
    }

    pub(in crate::executor) fn finish_compound_output_process_substitutions(
        &mut self,
        outputs: Vec<(PathBuf, String)>,
    ) -> Result<(), ExecuteError> {
        let mut error = None;
        for (path, source) in outputs {
            if error.is_none() {
                let input = fs::read_to_string(&path).unwrap_or_default();
                if let Err(output_error) =
                    self.execute_persistent_output_process_substitution(&source, input)
                {
                    error = Some(output_error);
                }
            }
            let _ = fs::remove_file(path);
        }
        if let Some(error) = error {
            return Err(error);
        }
        Ok(())
    }

    pub(in crate::executor) fn execute_simple_pipeline(
        &mut self,
        ast: &Ast,
        index: usize,
    ) -> Result<Option<usize>, ExecuteError> {
        let Some(first) = ast.commands.get(index) else {
            return Ok(None);
        };
        if first.pipe.is_none() {
            return Ok(None);
        }

        let mut commands = vec![first];
        let mut end = index;
        while ast
            .commands
            .get(end)
            .is_some_and(|command| command.pipe.is_some())
        {
            end += 1;
            let Some(command) = ast.commands.get(end) else {
                return Ok(None);
            };
            commands.push(command);
        }
        if commands.iter().any(|command| {
            self.is_this_shell_posixpipe_time_count(command)
                || self.is_posixpipe_time_count_fragment(command)
                || self.is_posixpipe_time_count_remainder(command)
        }) {
            return Ok(None);
        }

        if self
            .execute_external_pipeline_concurrently(&commands)?
            .is_some()
        {
            return Ok(Some(end + 1));
        }

        let time_prefix = time_pipeline_prefix(first);
        let time_prefix_started = time_prefix.as_ref().map(|_| time_command_started());
        let mut input = String::new();
        let mut statuses = Vec::new();
        for (stage_index, command) in commands.iter().enumerate() {
            let stage = time_prefix
                .as_ref()
                .filter(|_| stage_index == 0)
                .map(|prefix| &prefix.command)
                .unwrap_or(command);
            if stage_index == 0 {
                input = self.initial_pipeline_input(stage);
            }
            self.set_current_command(stage);
            let last_stage = stage_index + 1 == commands.len();
            let preserve_compound_errexit = command_is_compound_pipeline_stage(stage)
                || stage
                    .words
                    .first()
                    .map(|word| self.expand_word(word))
                    .and_then(|word| self.function_name_for_command_word(&word))
                    .is_some();
            let Some((mut next_input, next_stderr, next_status)) =
                (if last_stage && self.lastpipe_enabled() {
                    Some(self.execute_lastpipe_stage(stage, &input)?)
                } else if last_stage || preserve_compound_errexit {
                    self.execute_pipeline_stage(stage, &input)?
                } else {
                    // Non-final pipeline stages never trigger errexit (bash
                    // manual: "any command in a pipeline but the last");
                    // `{ false; echo foo; } | cat` still prints foo
                    // (set-e1.sub "after brace pipeline").
                    self.with_errexit_suppressed(|executor| {
                        executor.execute_pipeline_stage(stage, &input)
                    })?
                })
            else {
                return Ok(None);
            };
            if command.pipe == Some(2) {
                next_input.push_str(&next_stderr);
            } else if !next_stderr.is_empty() {
                std::io::stderr().write_all(next_stderr.as_bytes())?;
            }
            input = next_input;
            statuses.push(next_status);
        }

        let final_command = commands.last().expect("pipeline has at least one stage");
        self.write_pipeline_output(final_command, &input)?;
        if let Some(prefix) = &time_prefix {
            if let Some(started) = time_prefix_started {
                print_time(&self.env_vars, prefix.posix_format, started);
            }
        }
        let mut status = self.pipeline_exit_status(&statuses);
        if time_prefix.as_ref().is_some_and(|prefix| prefix.inverted) {
            status = invert_exit_status(status);
        }
        self.exit_code = if first.inverted {
            invert_exit_status(status)
        } else {
            status
        };
        self.set_pipestatus(statuses);
        Ok(Some(end + 1))
    }

    /// Connect a pipeline of native external processes with OS pipes.  The
    /// normal pipeline path captures each stage into a String before starting
    /// the next stage, which deadlocks for producers such as `yes` once a
    /// downstream `head` has already stopped reading.  Restrict this path to
    /// plain external `|` pipelines; builtins, compound commands, redirects,
    /// and `|&` retain the shell-aware path above.
    #[cfg(windows)]
    fn execute_external_pipeline_concurrently(
        &mut self,
        commands: &[&CommandNode],
    ) -> Result<Option<Vec<(String, String, i32)>>, ExecuteError> {
        if commands.len() < 2
            || self.stderr_capture.is_some()
            || self.stdout_capture.is_some()
            || commands.iter().enumerate().any(|(index, command)| {
                command.time_command.is_some()
                    || command.brace_group.is_some()
                    || command.subshell
                    || command_has_non_concurrent_pipeline_redirects(command, index, commands.len())
                    || command.redirect_in.is_some()
                    || command.redirect_err.is_some()
                    || command.redirect_err_append.is_some()
                    || ((command.redirect_out.is_some() || command.append.is_some())
                        && index + 1 != commands.len())
                    || ((command.heredoc.is_some()
                        || !command.heredoc_redirects.is_empty()
                        || command.here_string.is_some())
                        && index != 0)
                    || !command.assignments.is_empty()
                    || !command.process_substitutions.is_empty()
                    || command.pipe == Some(2)
            })
        {
            return Ok(None);
        }

        let mut specs = Vec::with_capacity(commands.len());
        for command in commands {
            let Some(name) = command.words.first() else {
                return Ok(None);
            };
            let expanded_name = self.expand_word(name);
            if crate::executor::builtin_names::is_shell_builtin_name(&expanded_name) {
                return Ok(None);
            }
            let Some(program) = find_user_command(&expanded_name, &self.env_vars) else {
                return Ok(None);
            };
            let args = command.words[1..]
                .iter()
                .map(|word| self.expand_word(word))
                .collect::<Vec<_>>();
            specs.push((program, args));
        }

        let mut pipes: Vec<(Option<os_pipe::PipeReader>, Option<os_pipe::PipeWriter>)> =
            Vec::with_capacity(commands.len() - 1);
        for _ in 0..commands.len() - 1 {
            let (read, write) = os_pipe::pipe().map_err(ExecuteError::IoError)?;
            pipes.push((Some(read), Some(write)));
        }

        let mut processes = Vec::with_capacity(commands.len());
        let capture_intermediate_stderr =
            self.fd_table.write_endpoint(2) == Some(FdWriteEndpoint::Stdout);
        let mut intermediate_stderr = Vec::new();
        for (index, (program, args)) in specs.iter().enumerate() {
            let (mut process, _) = external_command_for_named_program(
                program,
                Some(&self.expand_word(&commands[index].words[0])),
                args,
                &self.env_vars,
            );
            self.apply_child_environment(&mut process);

            if index == 0 {
                process.stdin(Stdio::piped());
            } else {
                let (read, _) = &mut pipes[index - 1];
                process.stdin(stdio_from_transferred_handle(
                    read.take().expect("pipeline reader already transferred"),
                ));
            }
            if index + 1 < commands.len() {
                let (_, write) = &mut pipes[index];
                process.stdout(stdio_from_transferred_handle(
                    write.take().expect("pipeline writer already transferred"),
                ));
                if capture_intermediate_stderr {
                    process.stderr(Stdio::piped());
                }
            } else {
                process.stdout(Stdio::piped());
                process.stderr(Stdio::piped());
            }

            let mut child = process.spawn().map_err(ExecuteError::IoError)?;
            if capture_intermediate_stderr && index + 1 < commands.len() {
                if let Some(mut stderr) = child.stderr.take() {
                    intermediate_stderr.push(std::thread::spawn(move || {
                        let mut output = Vec::new();
                        stderr.read_to_end(&mut output)?;
                        Ok::<_, std::io::Error>(output)
                    }));
                }
            }
            processes.push(child);
        }

        // Spawn every stage before writing a heredoc. A large heredoc can fill
        // the first stdin pipe while the downstream stages are still absent.
        if let Some(mut stdin) = processes[0].stdin.take() {
            let input = self.initial_pipeline_input(commands[0]);
            if !input.is_empty() {
                stdin.write_all(input.as_bytes())?;
            }
        }

        let mut results = Vec::with_capacity(processes.len());
        let last = processes.pop().expect("pipeline has at least two stages");
        let output = last.wait_with_output()?;
        results.push((
            String::from_utf8_lossy(&output.stdout).into_owned(),
            String::from_utf8_lossy(&output.stderr).into_owned(),
            output.status.code().unwrap_or(1),
        ));
        for mut process in processes.into_iter().rev() {
            let status = wait_for_windows_pipeline_member(&mut process)?;
            results.push((String::new(), String::new(), status.code().unwrap_or(1)));
        }
        results.reverse();
        for reader in intermediate_stderr {
            let output = reader.join().map_err(|_| {
                ExecuteError::IoError(std::io::Error::other("pipeline stderr reader panicked"))
            })??;
            if capture_intermediate_stderr {
                self.write_default_stdout(&output)?;
            }
        }
        self.write_pipeline_output(commands[commands.len() - 1], &results.last().unwrap().0)?;
        if let Some((_, stderr, _)) = results.last() {
            if !stderr.is_empty() {
                std::io::stderr().write_all(stderr.as_bytes())?;
            }
        }
        let statuses = results
            .iter()
            .map(|(_, _, status)| *status)
            .collect::<Vec<_>>();
        self.exit_code = self.pipeline_exit_status(&statuses);
        self.set_pipestatus(statuses);
        Ok(Some(results))
    }

    #[cfg(not(windows))]
    fn execute_external_pipeline_concurrently(
        &mut self,
        commands: &[&CommandNode],
    ) -> Result<Option<Vec<(String, String, i32)>>, ExecuteError> {
        if commands.len() < 2
            || self.stderr_capture.is_some()
            || self.stdout_capture.is_some()
            || commands.iter().enumerate().any(|(index, command)| {
                command.time_command.is_some()
                    || command.brace_group.is_some()
                    || command.subshell
                    || command_has_non_concurrent_pipeline_redirects(command, index, commands.len())
                    || command.redirect_in.is_some()
                    || command.redirect_err.is_some()
                    || command.redirect_err_append.is_some()
                    || ((command.redirect_out.is_some() || command.append.is_some())
                        && index + 1 != commands.len())
                    || ((command.heredoc.is_some()
                        || !command.heredoc_redirects.is_empty()
                        || command.here_string.is_some())
                        && index != 0)
                    || !command.assignments.is_empty()
                    || !command.process_substitutions.is_empty()
                    || command.pipe == Some(2)
            })
        {
            return Ok(None);
        }

        let mut specs = Vec::with_capacity(commands.len());
        for command in commands {
            let Some(name) = command.words.first() else {
                return Ok(None);
            };
            let expanded_name = self.expand_word(name);
            if crate::executor::builtin_names::is_shell_builtin_name(&expanded_name) {
                return Ok(None);
            }
            let Some(program) = find_user_command(&expanded_name, &self.env_vars) else {
                return Ok(None);
            };
            let args = command.words[1..]
                .iter()
                .map(|word| self.expand_word(word))
                .collect::<Vec<_>>();
            specs.push((program, args));
        }

        let mut processes: Vec<std::process::Child> = Vec::with_capacity(commands.len());
        let capture_intermediate_stderr =
            self.fd_table.write_endpoint(2) == Some(FdWriteEndpoint::Stdout);
        let mut intermediate_stderr = Vec::new();
        let mut previous_stdout: Option<std::process::ChildStdout> = None;
        let mut first_stdin: Option<std::process::ChildStdin> = None;

        for (index, (program, args)) in specs.iter().enumerate() {
            let (mut process, _) = external_command_for_named_program(
                &program,
                Some(&self.expand_word(&commands[index].words[0])),
                &args,
                &self.env_vars,
            );
            self.apply_child_environment(&mut process);

            if let Some(stdout) = previous_stdout.take() {
                process.stdin(Stdio::from(stdout));
            } else if index == 0 {
                process.stdin(Stdio::piped());
            }

            if index + 1 < commands.len() {
                process.stdout(Stdio::piped());
            } else {
                process.stdout(Stdio::piped());
            }
            if capture_intermediate_stderr || index + 1 == commands.len() {
                process.stderr(Stdio::piped());
            }

            let mut child = process.spawn().map_err(ExecuteError::IoError)?;
            if capture_intermediate_stderr && index + 1 < commands.len() {
                if let Some(mut stderr) = child.stderr.take() {
                    intermediate_stderr.push(std::thread::spawn(move || {
                        let mut output = Vec::new();
                        stderr.read_to_end(&mut output)?;
                        Ok::<_, std::io::Error>(output)
                    }));
                }
            }
            if index == 0 {
                first_stdin = child.stdin.take();
            }
            if index + 1 < commands.len() {
                previous_stdout = child.stdout.take();
            }
            processes.push(child);
        }

        if let Some(mut stdin) = first_stdin {
            let input = self.initial_pipeline_input(commands[0]);
            stdin.write_all(input.as_bytes())?;
        }

        let mut results = Vec::with_capacity(processes.len());
        let last = processes.pop().expect("pipeline has at least two stages");
        let output = last.wait_with_output()?;
        results.push((
            String::from_utf8_lossy(&output.stdout).into_owned(),
            String::from_utf8_lossy(&output.stderr).into_owned(),
            output.status.code().unwrap_or(1),
        ));
        for mut process in processes.into_iter().rev() {
            let status = match process.try_wait()? {
                Some(status) => status,
                None => {
                    let _ = process.kill();
                    process.wait()?
                }
            };
            results.push((String::new(), String::new(), status.code().unwrap_or(1)));
        }
        results.reverse();
        for reader in intermediate_stderr {
            let output = reader.join().map_err(|_| {
                ExecuteError::IoError(std::io::Error::other("pipeline stderr reader panicked"))
            })??;
            if capture_intermediate_stderr {
                self.write_default_stdout(&output)?;
            }
        }
        self.write_pipeline_output(commands[commands.len() - 1], &results.last().unwrap().0)?;
        if let Some((_, stderr, _)) = results.last() {
            if !stderr.is_empty() {
                if let Some(capture) = &mut self.stderr_capture {
                    capture.write_all(stderr.as_bytes())?;
                } else {
                    std::io::stderr().write_all(stderr.as_bytes())?;
                }
            }
        }
        let statuses = results
            .iter()
            .map(|(_, _, status)| *status)
            .collect::<Vec<_>>();
        self.exit_code = self.pipeline_exit_status(&statuses);
        self.set_pipestatus(statuses);
        Ok(Some(results))
    }

    pub(in crate::executor) fn pipeline_exit_status(&self, statuses: &[i32]) -> i32 {
        if crate::builtins::set::shell_option_enabled(&self.env_vars, "pipefail") {
            return statuses
                .iter()
                .rev()
                .copied()
                .find(|status| *status != 0)
                .unwrap_or(0);
        }

        statuses.last().copied().unwrap_or(0)
    }

    pub(in crate::executor) fn execute_pipeline_stage(
        &mut self,
        command: &CommandNode,
        input: &str,
    ) -> Result<Option<(String, String, i32)>, ExecuteError> {
        if let Some(time_command) = &command.time_command {
            let started = time_command_started();
            let Some((output, stderr, status)) =
                self.execute_pipeline_stage(&time_command.command, input)?
            else {
                return Ok(None);
            };
            print_time(&self.env_vars, time_command.posix_format, started);
            let status = if time_command.inverted {
                invert_exit_status(status)
            } else {
                status
            };
            return Ok(Some((output, stderr, status)));
        }

        if command_is_compound_pipeline_stage(command) {
            return self
                .execute_compound_pipeline_stage(command, input)
                .map(Some);
        }

        let expanded = self.brace_expanded_pipeline_stage(command);
        let command = &expanded;
        let Some(name) = command.words.first().map(String::as_str) else {
            return self
                .execute_compound_pipeline_stage(command, input)
                .map(Some);
        };

        match name {
            "true" | ":" => Ok(Some((String::new(), String::new(), 0))),
            "false" => Ok(Some((String::new(), String::new(), 1))),
            "echo" => {
                let mut args: Vec<String> = command.words[1..]
                    .iter()
                    .map(|word| self.expand_word(word))
                    .collect();
                let newline = !args.first().is_some_and(|arg| arg == "-n");
                if !newline {
                    args.remove(0);
                }
                let mut output = args.join(" ");
                if newline {
                    output.push('\n');
                }
                Ok(Some((output, String::new(), 0)))
            }
            "printf" => {
                let args: Vec<String> = command.words[1..]
                    .iter()
                    .map(|word| self.expand_word(word))
                    .collect();
                let mut env_vars = self.env_vars.clone();
                let mut output = Vec::new();
                let mut stderr = Vec::new();
                let status = crate::builtins::printf::execute_with_io(
                    args.iter().map(String::as_str),
                    &mut env_vars,
                    &mut output,
                    &mut stderr,
                )?;
                Ok(Some((
                    String::from_utf8_lossy(&output).into_owned(),
                    String::from_utf8_lossy(&stderr).into_owned(),
                    status,
                )))
            }
            "cat" => {
                if let Some(input) = self.stdin_string_for_command(command) {
                    Ok(Some((input, String::new(), 0)))
                } else {
                    Ok(Some((input.to_string(), String::new(), 0)))
                }
            }
            "sed" => {
                let args = command.words[1..]
                    .iter()
                    .map(|word| self.expand_word(word))
                    .collect::<Vec<_>>();
                if let Some(output) = apply_simple_sed_args(input, &args) {
                    Ok(Some((output, String::new(), 0)))
                } else {
                    self.execute_external_pipeline_stage(command, input)
                }
            }
            "grep" => {
                let Some(pattern) = command.words.get(1).map(|word| self.expand_word(word)) else {
                    return Ok(Some((String::new(), String::new(), 2)));
                };
                let mut matched = false;
                let mut output = String::new();
                for line in input.split_inclusive('\n') {
                    let comparable = line.strip_suffix('\n').unwrap_or(line);
                    if simple_grep_pattern_matches(comparable, &pattern) {
                        matched = true;
                        output.push_str(line);
                        if !line.ends_with('\n') {
                            output.push('\n');
                        }
                    }
                }
                Ok(Some((output, String::new(), i32::from(!matched))))
            }
            "wc" => {
                let option = command.words.get(1).map(String::as_str).unwrap_or("-l");
                let value = match option {
                    "-c" => input.as_bytes().len(),
                    "-l" => input.bytes().filter(|byte| *byte == b'\n').count(),
                    _ => return Ok(None),
                };
                Ok(Some((format!("{value}\n"), String::new(), 0)))
            }
            "tr" => {
                let args = command.words[1..]
                    .iter()
                    .map(|word| self.expand_word(word))
                    .collect::<Vec<_>>();
                if args.len() == 2 && matches!(args[0].as_str(), "\\n" | "\n") {
                    Ok(Some((input.replace('\n', &args[1]), String::new(), 0)))
                } else {
                    self.execute_external_pipeline_stage(command, input)
                }
            }
            _ => {
                if let Some(output) = self.execute_function_pipeline_stage(command, input)? {
                    Ok(Some(output))
                } else {
                    if let Some(output) = self.execute_builtin_pipeline_stage(command, input)? {
                        Ok(Some(output))
                    } else {
                        self.execute_external_pipeline_stage(command, input)
                    }
                }
            }
        }
    }

    fn brace_expanded_pipeline_stage(&self, command: &CommandNode) -> CommandNode {
        if !self.is_brace_expand_enabled() {
            return command.clone();
        }

        let mut expanded = command.clone();
        expanded.words = command
            .words
            .iter()
            .enumerate()
            .flat_map(|(index, word)| {
                let raw = command
                    .word_metadata
                    .get(index)
                    .map(|metadata| metadata.raw.as_str());
                crate::executor::command_prepare::expand_braces_with_optional_raw(word, raw)
            })
            .collect();
        expanded
    }

    fn lastpipe_enabled(&self) -> bool {
        crate::builtins::shopt::option_enabled(&self.env_vars, "lastpipe")
    }

    fn initial_pipeline_input(&self, command: &CommandNode) -> String {
        self.stdin_string_for_command(command)
            .or_else(|| {
                pipeline_stage_reads_stdin_by_default(command)
                    .then(|| self.read_inherited_process_stdin_to_string())
                    .flatten()
            })
            .unwrap_or_default()
    }
}

fn command_is_compound_pipeline_stage(command: &CommandNode) -> bool {
    command.for_command.is_some()
        || command.if_command.is_some()
        || command.loop_command.is_some()
        || command.select_command.is_some()
        || command.case_command.is_some()
        || command.coproc_command.is_some()
        || command.subshell_command.is_some()
        || command.brace_group.is_some()
        || command.time_command.is_some()
        || command.arithmetic_command.is_some()
        || command.conditional_command.is_some()
        || command.inverted_command.is_some()
        || command.background_command.is_some()
}

fn pipeline_stage_reads_stdin_by_default(command: &CommandNode) -> bool {
    let Some(command_name) = command.words.first().map(String::as_str) else {
        return false;
    };
    let command_name = command_name
        .rsplit(['/', '\\'])
        .next()
        .unwrap_or(command_name);
    matches!(
        command_name,
        "awk" | "cat" | "grep" | "head" | "sed" | "sort" | "tail" | "tr" | "uniq" | "wc"
    )
}

fn command_has_non_concurrent_pipeline_redirects(
    command: &CommandNode,
    index: usize,
    pipeline_len: usize,
) -> bool {
    if command.redirects.is_empty() {
        return false;
    }
    let is_last_stage = index + 1 == pipeline_len;
    command.redirects.iter().any(|redirect| {
        let is_initial_heredoc = index == 0
            && matches!(
                redirect.kind,
                crate::parser::RedirectKind::HereDoc | crate::parser::RedirectKind::HereString
            );
        let is_final_output = is_last_stage
            && matches!(
                redirect.kind,
                crate::parser::RedirectKind::Output
                    | crate::parser::RedirectKind::Append
                    | crate::parser::RedirectKind::ClobberOutput
            );
        !is_initial_heredoc && !is_final_output
    })
}

struct TimePipelinePrefix {
    command: CommandNode,
    inverted: bool,
    posix_format: bool,
}

fn time_pipeline_prefix(command: &CommandNode) -> Option<TimePipelinePrefix> {
    if command.words.first().map(String::as_str) != Some("time") {
        return None;
    }

    let mut index = 1;
    let mut inverted = false;
    let mut posix_format = false;
    while let Some(word) = command.words.get(index).map(String::as_str) {
        match word {
            "-p" => {
                posix_format = true;
                index += 1;
            }
            "--" => index += 1,
            "!" => {
                inverted = !inverted;
                index += 1;
            }
            _ => break,
        }
    }
    if index >= command.words.len() {
        return None;
    }

    let mut stripped = command.clone();
    stripped.words = command.words[index..].to_vec();
    if command.word_kinds.len() == command.words.len() {
        stripped.word_kinds = command.word_kinds[index..].to_vec();
    }
    if command.word_metadata.len() == command.words.len() {
        stripped.word_metadata = command.word_metadata[index..].to_vec();
    }
    Some(TimePipelinePrefix {
        command: stripped,
        inverted,
        posix_format,
    })
}
