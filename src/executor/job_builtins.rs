use super::*;
use std::io::Write;

impl Executor {
    pub(in crate::executor) fn execute_times(
        &mut self,
        cmd: &CommandNode,
    ) -> Result<i32, ExecuteError> {
        let mut stdout = Vec::new();
        let mut stderr = Vec::new();
        let status = crate::builtins::times::execute_with_io(
            cmd.words[1..].iter().map(String::as_str),
            &mut stdout,
            &mut stderr,
        )?;
        self.write_buffered_builtin_output(cmd, &stdout, &stderr)?;
        Ok(status)
    }

    pub(in crate::executor) fn execute_caller(
        &mut self,
        cmd: &CommandNode,
    ) -> Result<i32, ExecuteError> {
        let funcname = self.funcname_stack();
        let lineno = self.indexed_array_stack("BASH_LINENO");
        // The executor uses `main` as the synthetic source name for function
        // calls made from an inline command string.  Bash's `caller` builtin
        // reports that frame as `environment`, while BASH_SOURCE itself keeps
        // the internal synthetic name for compatibility with the shell API.
        let source: Vec<String> = self
            .indexed_array_stack("BASH_SOURCE")
            .into_iter()
            .map(|name| {
                if name == "main" {
                    "environment".to_string()
                } else {
                    name
                }
            })
            .collect();
        let mut stdout = Vec::new();
        let mut stderr = Vec::new();
        let status = crate::builtins::caller::execute_with_io(
            &cmd.words[1..],
            &funcname,
            &lineno,
            &source,
            &self.diagnostic_prefix(),
            &mut stdout,
            &mut stderr,
        )?;
        self.write_buffered_builtin_output(cmd, &stdout, &stderr)?;
        Ok(status)
    }

    pub(in crate::executor) fn execute_jobs(
        &mut self,
        cmd: &CommandNode,
    ) -> Result<i32, ExecuteError> {
        self.refresh_background_jobs()?;
        let mut stderr = Vec::new();
        let action = crate::builtins::jobs::execute_with_io(
            &cmd.words[1..],
            &self.diagnostic_prefix(),
            &mut stderr,
        )?;
        match action {
            crate::builtins::jobs::JobsAction::Complete(status) => {
                self.write_buffered_builtin_output(cmd, &[], &stderr)?;
                Ok(status)
            }
            crate::builtins::jobs::JobsAction::List { options, jobs } => {
                let (stdout, status) = self.background_jobs_output(options, &jobs, &mut stderr)?;
                self.write_buffered_builtin_output(cmd, stdout.as_bytes(), &stderr)?;
                Ok(status)
            }
            crate::builtins::jobs::JobsAction::Execute(words) => {
                if !stderr.is_empty() {
                    self.write_buffered_builtin_output(cmd, &[], &stderr)?;
                    return Ok(1);
                }
                let Some(words) = self.expand_jobs_x_words(words, &mut stderr)? else {
                    self.write_buffered_builtin_output(cmd, &[], &stderr)?;
                    return Ok(1);
                };
                let mut command = cmd.clone();
                command.words = words;
                self.execute_command(&command)?;
                Ok(self.exit_code)
            }
        }
    }

    fn expand_jobs_x_words(
        &self,
        words: Vec<String>,
        stderr: &mut Vec<u8>,
    ) -> Result<Option<Vec<String>>, ExecuteError> {
        let mut expanded = Vec::with_capacity(words.len());
        for word in words {
            if word.starts_with('%') {
                let Some(pid) = self.resolve_background_job(&word) else {
                    writeln!(
                        stderr,
                        "{}jobs: {word}: no such job",
                        self.diagnostic_prefix()
                    )?;
                    return Ok(None);
                };
                expanded.push(pid.to_string());
            } else {
                expanded.push(word);
            }
        }
        Ok(Some(expanded))
    }

    pub(in crate::executor) fn execute_wait(
        &mut self,
        cmd: &CommandNode,
    ) -> Result<i32, ExecuteError> {
        self.refresh_background_jobs()?;
        if let Some((pid, wait_var)) = self.wait_any_background_request(cmd) {
            // wait -n consumes the selected completion; explicit waits retain
            // it so a repeated wait for the same pid returns the same status.
            if let Some(status) = self.wait_for_background_pid(pid, false)? {
                if let Some(wait_var) = wait_var {
                    self.apply_shell_assignment(&wait_var, pid.to_string());
                }
                self.write_buffered_builtin_output(cmd, &[], &[])?;
                return Ok(status);
            }
        }

        if cmd.words.len() == 1 && self.job_table.jobs.values().any(|job| job.background) {
            let pids = self
                .job_table
                .jobs
                .values()
                .filter(|job| job.background)
                .flat_map(|job| job.pids.iter().copied())
                .collect::<Vec<_>>();
            for pid in pids {
                let _ = self.wait_for_background_pid(pid, false)?;
            }
            self.write_buffered_builtin_output(cmd, &[], &[])?;
            // Bash's no-operand wait reports success after waiting for all
            // current jobs; individual statuses require an explicit operand.
            return Ok(0);
        }

        if let Some(operands) = wait_background_operands(&cmd.words[1..]) {
            if !operands.is_empty()
                && operands
                    .iter()
                    .any(|operand| self.resolve_background_job(operand).is_some())
            {
                let status = self.wait_for_background_operands(&operands, cmd)?;
                return Ok(status);
            }
        }

        if cmd.words.len() == 2 {
            if let Some(pid) = self.resolve_background_job(&cmd.words[1]) {
                if let Some(status) = self.wait_for_background_pid(pid, true)? {
                    self.write_buffered_builtin_output(cmd, &[], &[])?;
                    return Ok(status);
                }
            } else if let Ok(pid) = cmd.words[1].parse::<u32>() {
                if let Some(status) = self.wait_for_background_pid(pid, true)? {
                    self.write_buffered_builtin_output(cmd, &[], &[])?;
                    return Ok(status);
                }
            }
        }

        let mut stderr = Vec::new();
        let status = crate::builtins::wait::execute_with_io(
            &cmd.words[1..],
            &self.diagnostic_prefix(),
            &mut stderr,
        )?;
        self.write_buffered_builtin_output(cmd, &[], &stderr)?;
        Ok(status)
    }

    fn wait_for_background_operands(
        &mut self,
        operands: &[String],
        cmd: &CommandNode,
    ) -> Result<i32, ExecuteError> {
        let resolved = operands
            .iter()
            .map(|operand| (operand.clone(), self.resolve_background_job(operand)))
            .collect::<Vec<_>>();
        let mut stderr = Vec::new();
        let mut status = 0;

        for (operand, pid) in resolved {
            let Some(pid) = pid else {
                status =
                    write_wait_operand_error(&operand, &self.diagnostic_prefix(), &mut stderr)?;
                continue;
            };
            if let Some(wait_status) = self.wait_for_background_pid(pid, true)? {
                status = wait_status;
            } else {
                status =
                    write_wait_operand_error(&operand, &self.diagnostic_prefix(), &mut stderr)?;
            }
        }

        self.write_buffered_builtin_output(cmd, &[], &stderr)?;
        Ok(status)
    }

    fn wait_any_background_request(&self, cmd: &CommandNode) -> Option<(u32, Option<String>)> {
        let request = wait_any_request(&cmd.words[1..])?;
        let pid = if let Some(first) = request.operands.first() {
            self.resolve_background_job(first)?
        } else {
            // A completed child may no longer have a Child handle, but its
            // status remains in JobTable until an explicit wait consumes it.
            self.job_table
                .completed_statuses
                .keys()
                .next()
                .copied()
                .or_else(|| {
                    self.job_table
                        .jobs
                        .values()
                        .filter(|job| {
                            job.background && job.state != crate::jobs::ProcessState::Completed
                        })
                        .filter_map(|job| job.pids.last().copied())
                        .next()
                })?
        };
        Some((pid, request.assign_var))
    }

    pub(in crate::executor) fn refresh_background_jobs(&mut self) -> Result<(), ExecuteError> {
        self.refresh_background_jobs_with_protected_coprocs(&[])
    }

    pub(in crate::executor) fn refresh_background_jobs_with_protected_coprocs(
        &mut self,
        protected_coprocs: &[u32],
    ) -> Result<(), ExecuteError> {
        let mut finished = Vec::new();
        for (pid, child) in &mut self.background_children {
            if let Some(status) = child.try_wait()? {
                finished.push((*pid, status.code().unwrap_or(1)));
            }
        }

        for (pid, status) in finished {
            self.background_children.remove(&pid);
            self.join_coproc_stderr_forwarder(pid)?;
            self.job_table.mark_completed(pid, status);
            if !protected_coprocs.contains(&pid) {
                self.retire_completed_coproc(pid);
            }
        }
        Ok(())
    }

    fn join_coproc_stderr_forwarder(&mut self, pid: u32) -> Result<(), ExecuteError> {
        let Some(forwarder) = self.coproc_stderr_forwarders.remove(&pid) else {
            return Ok(());
        };
        let result = forwarder.join().map_err(|_| {
            ExecuteError::IoError(std::io::Error::other("coprocess stderr forwarder panicked"))
        })?;
        result.map_err(ExecuteError::IoError)
    }

    fn retire_completed_coproc(&mut self, pid: u32) {
        let is_coproc = self.coproc_stdin_writers.contains_key(&pid)
            || self.coproc_stdout_readers.contains_key(&pid)
            || self.fd_table.entries.values().any(|entry| {
                matches!(
                    entry.read.as_ref(),
                    Some(FdReadEndpoint::CoprocStdout(endpoint_pid)) if *endpoint_pid == pid
                ) || matches!(
                    entry.write.as_ref(),
                    Some(FdWriteEndpoint::CoprocStdin(endpoint_pid)) if *endpoint_pid == pid
                )
            });
        if !is_coproc {
            return;
        }

        self.coproc_stdin_writers.remove(&pid);
        self.coproc_stdout_readers.remove(&pid);

        let endpoint_fds = self
            .fd_table
            .entries
            .iter()
            .filter_map(|(fd, entry)| {
                let matches_read = matches!(
                    entry.read.as_ref(),
                    Some(FdReadEndpoint::CoprocStdout(endpoint_pid)) if *endpoint_pid == pid
                );
                let matches_write = matches!(
                    entry.write.as_ref(),
                    Some(FdWriteEndpoint::CoprocStdin(endpoint_pid)) if *endpoint_pid == pid
                );
                (matches_read || matches_write).then_some(*fd)
            })
            .collect::<Vec<_>>();
        for fd in endpoint_fds {
            self.fd_table.close(fd);
            self.env_vars.remove(&fd_stdin_key(fd));
            self.env_vars.remove(&fd_stdin_offset_key(fd));
            self.env_vars.remove(&fd_dynamic_input_key(fd));
            self.env_vars.remove(&fd_output_key(fd));
            self.env_vars
                .remove(&fd_output_process_substitution_key(fd));
            self.env_vars.insert(fd_closed_key(fd), "1".to_string());
        }
        let coproc_names = self
            .env_vars
            .iter()
            .filter_map(|(key, value)| {
                key.strip_suffix("_PID")
                    .filter(|_| value == &pid.to_string())
                    .map(str::to_string)
            })
            .collect::<Vec<_>>();
        for name in coproc_names {
            self.env_vars.remove(&name);
            self.env_vars.remove(&format!("{name}_PID"));
            unmark_env_name(&mut self.env_vars, ARRAY_VARS, &name);
        }

        let coproc_prefix = format!("{FD_COPROC_STDIN_TARGET_PREFIX}{pid}");
        self.env_vars.retain(|key, value| {
            !((key.starts_with(FD_STDIN_PREFIX) || key.starts_with(FD_OUTPUT_PREFIX))
                && value == &coproc_prefix)
        });
    }

    fn forget_background_runtime(&mut self, pid: u32) {
        self.background_children.remove(&pid);
        self.background_jobs.remove(&pid);
        self.background_job_order.retain(|job_pid| *job_pid != pid);
        self.coproc_stdin_writers.remove(&pid);
        self.coproc_stdout_readers.remove(&pid);
        self.fd_table.close(pid);
    }

    fn wait_for_background_pid(
        &mut self,
        pid: u32,
        _retain_for_explicit_wait: bool,
    ) -> Result<Option<i32>, ExecuteError> {
        if let Some(status) = self.job_table.completed_statuses.get(&pid).copied() {
            self.join_coproc_stderr_forwarder(pid)?;
            // Waiting consumes the jobs-table entry, but the completed status
            // remains available for a later explicit wait of the same PID.
            self.job_table.remove_job_by_pid_preserve_status(pid);
            self.forget_background_runtime(pid);
            return Ok(Some(status));
        }
        let Some(mut child) = self.background_children.remove(&pid) else {
            return Ok(None);
        };
        let status = child.wait()?.code().unwrap_or(1);
        self.join_coproc_stderr_forwarder(pid)?;
        self.job_table.mark_completed(pid, status);
        // Remove the visible job after any wait, while retaining the exit
        // status for repeated explicit PID waits.
        self.job_table.remove_job_by_pid_preserve_status(pid);
        self.forget_background_runtime(pid);
        Ok(Some(status))
    }

    fn background_jobs_output(
        &self,
        options: crate::builtins::jobs::JobsListOptions,
        requested_jobs: &[String],
        stderr: &mut Vec<u8>,
    ) -> Result<(String, i32), ExecuteError> {
        let jobs = if requested_jobs.is_empty() {
            self.ordered_background_jobs()
        } else {
            let mut selected = Vec::new();
            let mut status = 0;
            for job in requested_jobs {
                if let Some(pid) = self.resolve_background_job(job) {
                    if let Some(job_id) = self.job_table.pid_to_job.get(&pid).copied() {
                        if let Some(entry) = self.job_table.jobs.get(&job_id) {
                            selected.push((
                                self.background_job_number(pid),
                                pid,
                                entry.command.clone(),
                            ));
                        }
                    }
                } else {
                    writeln!(
                        stderr,
                        "{}jobs: {job}: no such job",
                        self.diagnostic_prefix()
                    )?;
                    status = 1;
                }
            }
            return Ok((self.render_background_jobs(options, selected), status));
        };
        Ok((self.render_background_jobs(options, jobs), 0))
    }

    fn ordered_background_jobs(&self) -> Vec<(usize, u32, String)> {
        self.job_table
            .jobs
            .values()
            .filter(|job| job.background)
            .enumerate()
            .filter_map(|(index, job)| {
                job.pids
                    .last()
                    .copied()
                    .map(|pid| (index + 1, pid, job.command.clone()))
            })
            .collect()
    }

    fn render_background_jobs(
        &self,
        options: crate::builtins::jobs::JobsListOptions,
        jobs: Vec<(usize, u32, String)>,
    ) -> String {
        let mut output = String::new();
        for (job_number, pid, source) in jobs {
            let state_text = self
                .job_table
                .pid_to_job
                .get(&pid)
                .and_then(|job_id| self.job_table.jobs.get(job_id))
                .map(|job| match job.state {
                    crate::jobs::ProcessState::Running => "Running".to_string(),
                    crate::jobs::ProcessState::Stopped => "Stopped".to_string(),
                    crate::jobs::ProcessState::Completed => match job.exit_status.unwrap_or(1) {
                        0 => "Done".to_string(),
                        status => format!("Exit {status}"),
                    },
                })
                .unwrap_or_else(|| "Unknown".to_string());
            if options.pids_only {
                output.push_str(&format!("{pid}\n"));
            } else if options.long {
                output.push_str(&format!(
                    "[{job_number}]  {pid} {state_text:<22} {source} &\n"
                ));
            } else {
                output.push_str(&format!("[{job_number}]  {state_text:<22} {source} &\n"));
            }
        }
        output
    }

    pub(in crate::executor) fn execute_disown(
        &mut self,
        cmd: &CommandNode,
    ) -> Result<i32, ExecuteError> {
        let mut stderr = Vec::new();
        let action = crate::builtins::disown::execute_with_io(
            &cmd.words[1..],
            &self.diagnostic_prefix(),
            &mut stderr,
        )?;
        let status = match action {
            crate::builtins::disown::DisownAction::Complete(status) => status,
            crate::builtins::disown::DisownAction::All => {
                let pids: Vec<u32> = self
                    .background_jobs
                    .keys()
                    .copied()
                    .chain(self.background_children.keys().copied())
                    .collect();
                self.background_children.clear();
                self.background_jobs.clear();
                self.background_job_order.clear();
                self.coproc_stdin_writers.clear();
                self.coproc_stdout_readers.clear();
                for pid in pids {
                    self.fd_table.close(pid);
                    self.job_table.remove_job_by_pid(pid);
                }
                self.job_table.clear_jobs();
                0
            }
            crate::builtins::disown::DisownAction::Current => {
                if self.disown_current_job() {
                    0
                } else {
                    writeln!(
                        stderr,
                        "{}disown: current: no such job",
                        self.diagnostic_prefix()
                    )?;
                    1
                }
            }
            crate::builtins::disown::DisownAction::Jobs(jobs) => {
                let mut status = 0;
                for job in jobs {
                    if let Some(pid) = self.resolve_background_job(&job) {
                        self.background_children.remove(&pid);
                        self.background_jobs.remove(&pid);
                        self.background_job_order.retain(|job_pid| *job_pid != pid);
                        self.coproc_stdin_writers.remove(&pid);
                        self.coproc_stdout_readers.remove(&pid);
                        self.fd_table.close(pid);
                        self.job_table.remove_job_by_pid(pid);
                    } else {
                        writeln!(
                            stderr,
                            "{}disown: {job}: no such job",
                            self.diagnostic_prefix()
                        )?;
                        status = 1;
                    }
                }
                status
            }
        };
        self.write_buffered_builtin_output(cmd, &[], &stderr)?;
        Ok(status)
    }

    fn disown_current_job(&mut self) -> bool {
        let Some(pid) = self.last_background_pid else {
            return false;
        };
        if !self.background_children.contains_key(&pid) && !self.background_jobs.contains_key(&pid)
        {
            return false;
        }

        self.background_children.remove(&pid);
        self.background_jobs.remove(&pid);
        self.background_job_order.retain(|job_pid| *job_pid != pid);
        self.coproc_stdin_writers.remove(&pid);
        self.coproc_stdout_readers.remove(&pid);
        self.fd_table.close(pid);
        self.job_table.remove_job_by_pid(pid);
        true
    }

    pub(in crate::executor) fn resolve_background_job(&self, job: &str) -> Option<u32> {
        if job.starts_with('%') {
            let job_id = self.job_table.resolve_jobspec(job)?;
            return self.job_table.jobs.get(&job_id)?.pids.last().copied();
        }
        let pid = job.parse::<u32>().ok()?;
        (self.job_table.pid_to_job.contains_key(&pid)
            || self.job_table.completed_statuses.contains_key(&pid))
        .then_some(pid)
    }

    fn background_job_number(&self, pid: u32) -> usize {
        self.job_table
            .pid_to_job
            .get(&pid)
            .and_then(|job_id| {
                self.job_table
                    .jobs
                    .keys()
                    .position(|candidate| candidate == job_id)
            })
            .map(|index| index + 1)
            .unwrap_or(1)
    }

    pub(in crate::executor) fn execute_fg_bg(
        &mut self,
        cmd: &CommandNode,
        builtin: crate::builtins::fg_bg::JobControlBuiltin,
    ) -> Result<i32, ExecuteError> {
        let mut stderr = Vec::new();
        let action = crate::builtins::fg_bg::execute_with_io(
            builtin,
            &cmd.words[1..],
            &self.diagnostic_prefix(),
            &mut stderr,
        )?;
        let has_job_control = self.job_table.jobs.values().any(|job| job.background);
        let status = match action {
            // Bash reports the non-interactive job-control failure before
            // validating fg/bg operands or options when no jobs exist.
            crate::builtins::fg_bg::FgBgAction::Complete(_status) if !has_job_control => {
                stderr.clear();
                crate::builtins::fg_bg::write_no_job_control(
                    builtin,
                    &self.diagnostic_prefix(),
                    &mut stderr,
                )?
            }
            crate::builtins::fg_bg::FgBgAction::Complete(status) => status,
            crate::builtins::fg_bg::FgBgAction::Jobs(jobs) => {
                if !has_job_control {
                    crate::builtins::fg_bg::write_no_job_control(
                        builtin,
                        &self.diagnostic_prefix(),
                        &mut stderr,
                    )?
                } else {
                    match builtin {
                        crate::builtins::fg_bg::JobControlBuiltin::Fg => {
                            self.execute_fg_jobs(jobs, &mut stderr)?
                        }
                        crate::builtins::fg_bg::JobControlBuiltin::Bg => {
                            self.execute_bg_jobs(jobs, &mut stderr)?
                        }
                    }
                }
            }
        };
        self.write_buffered_builtin_output(cmd, &[], &stderr)?;
        Ok(status)
    }

    fn execute_fg_jobs(
        &mut self,
        jobs: Vec<String>,
        stderr: &mut Vec<u8>,
    ) -> Result<i32, ExecuteError> {
        let job = jobs.first().map(String::as_str);
        let Some(pid) = self.resolve_requested_background_job(job) else {
            self.write_job_not_found("fg", job, stderr)?;
            return Ok(1);
        };

        let Some(mut child) = self.background_children.remove(&pid) else {
            self.background_jobs.remove(&pid);
            self.background_job_order.retain(|job_pid| *job_pid != pid);
            self.coproc_stdin_writers.remove(&pid);
            self.coproc_stdout_readers.remove(&pid);
            self.fd_table.close(pid);
            self.write_job_not_found("fg", job, stderr)?;
            return Ok(1);
        };
        self.background_jobs.remove(&pid);
        self.background_job_order.retain(|job_pid| *job_pid != pid);
        self.coproc_stdin_writers.remove(&pid);
        self.coproc_stdout_readers.remove(&pid);
        self.fd_table.close(pid);
        let status = child.wait()?.code().unwrap_or(1);
        self.job_table.mark_completed(pid, status);
        let status = self.job_table.wait_pid(pid).unwrap_or(status);
        self.job_table.remove_job_by_pid(pid);
        Ok(status)
    }

    fn execute_bg_jobs(
        &mut self,
        jobs: Vec<String>,
        stderr: &mut Vec<u8>,
    ) -> Result<i32, ExecuteError> {
        let requested = if jobs.is_empty() {
            vec![None]
        } else {
            jobs.iter()
                .map(|job| Some(job.as_str()))
                .collect::<Vec<_>>()
        };

        let mut status = 0;
        for job in requested {
            if let Some(pid) = self.resolve_requested_background_job(job) {
                self.job_table.mark_running(pid);
                if let Some(job_id) = self.job_table.pid_to_job.get(&pid).copied() {
                    if let Some(entry) = self.job_table.jobs.get_mut(&job_id) {
                        entry.background = true;
                        entry.foreground = false;
                    }
                }
            } else {
                self.write_job_not_found("bg", job, stderr)?;
                status = 1;
            }
        }
        Ok(status)
    }

    fn resolve_requested_background_job(&self, job: Option<&str>) -> Option<u32> {
        match job {
            Some(job) => self.resolve_background_job(job),
            None => self.current_background_pid(),
        }
    }

    fn current_background_pid(&self) -> Option<u32> {
        let job_id = self.job_table.current_job()?;
        self.job_table.jobs.get(&job_id)?.pids.last().copied()
    }

    fn write_job_not_found(
        &self,
        builtin: &str,
        job: Option<&str>,
        stderr: &mut Vec<u8>,
    ) -> Result<(), ExecuteError> {
        let job = job.unwrap_or("current");
        writeln!(
            stderr,
            "{}{builtin}: {job}: no such job",
            self.diagnostic_prefix()
        )?;
        Ok(())
    }

    pub(in crate::executor) fn execute_suspend(
        &mut self,
        cmd: &CommandNode,
    ) -> Result<i32, ExecuteError> {
        let mut stderr = Vec::new();
        let status = crate::builtins::suspend::execute_with_io(
            &cmd.words[1..],
            &self.diagnostic_prefix(),
            &mut stderr,
        )?;
        self.write_buffered_builtin_output(cmd, &[], &stderr)?;
        Ok(status)
    }

    pub(in crate::executor) fn execute_history(
        &mut self,
        cmd: &CommandNode,
    ) -> Result<i32, ExecuteError> {
        let mut stdout = Vec::new();
        let mut stderr = Vec::new();
        let args = &cmd.words[1..];
        if let Some(provider) = self.history_provider.as_ref() {
            let clear = args.iter().any(|arg| arg == "-c" || arg == "-pc");
            let save = args.iter().position(|arg| arg == "-s" || arg == "-ps");
            let delete = args.iter().position(|arg| arg == "-d");
            let entries = if clear {
                provider.borrow_mut().clear()?;
                Vec::new()
            } else if let Some(index) = save {
                let command = args[index + 1..].join(" ");
                if !command.is_empty() {
                    provider.borrow_mut().append(command)?;
                }
                Vec::new()
            } else if let Some(index) = delete {
                let mut entries = provider.borrow_mut().entries()?;
                if let Some(offset) = args
                    .get(index + 1)
                    .and_then(|value| value.parse::<usize>().ok())
                    .filter(|offset| *offset > 0)
                {
                    if offset <= entries.len() {
                        entries.remove(offset - 1);
                        provider.borrow_mut().replace(entries.clone())?;
                    }
                }
                Vec::new()
            } else {
                provider.borrow_mut().entries()?
            };
            let status = crate::builtins::history::execute_with_history(
                args,
                &self.diagnostic_prefix(),
                &entries,
                &mut stdout,
                &mut stderr,
            )?;
            self.write_buffered_builtin_output(cmd, &stdout, &stderr)?;
            return Ok(status);
        }
        let status = crate::builtins::history::execute_with_io(
            args,
            &self.diagnostic_prefix(),
            &mut stderr,
        )?;
        self.write_buffered_builtin_output(cmd, &stdout, &stderr)?;
        Ok(status)
    }

    pub(in crate::executor) fn execute_bind(
        &mut self,
        cmd: &CommandNode,
    ) -> Result<i32, ExecuteError> {
        let mut stderr = Vec::new();
        let status = crate::builtins::bind::execute_with_io(
            &cmd.words[1..],
            &self.diagnostic_prefix(),
            &mut stderr,
        )?;
        self.write_buffered_builtin_output(cmd, &[], &stderr)?;
        Ok(status)
    }

    pub(in crate::executor) fn execute_fc(
        &mut self,
        cmd: &CommandNode,
    ) -> Result<i32, ExecuteError> {
        let mut stdout = Vec::new();
        let mut stderr = Vec::new();
        let status = if let Some(provider) = self.history_provider.as_ref() {
            let entries = provider.borrow_mut().entries()?;
            crate::builtins::fc::execute_with_history(
                &cmd.words[1..],
                &self.diagnostic_prefix(),
                &entries,
                &mut stdout,
                &mut stderr,
            )?
        } else {
            crate::builtins::fc::execute_with_io(
                &cmd.words[1..],
                &self.diagnostic_prefix(),
                &mut stderr,
            )?
        };
        self.write_buffered_builtin_output(cmd, &stdout, &stderr)?;
        Ok(status)
    }

    pub(in crate::executor) fn execute_completion_builtin(
        &mut self,
        cmd: &CommandNode,
        builtin: crate::builtins::complete::CompletionBuiltin,
    ) -> Result<i32, ExecuteError> {
        let mut stdout = Vec::new();
        let mut stderr = Vec::new();
        let function_names: Vec<String> = self.functions.keys().cloned().collect();
        let job_names: Vec<String> = self
            .job_table
            .jobs
            .values()
            .filter(|job| job.background)
            .map(|job| job.command.clone())
            .collect();
        let status = crate::builtins::complete::execute_with_io(
            builtin,
            &cmd.words[1..],
            &self.env_vars,
            &self.aliases,
            &function_names,
            &job_names,
            &self.diagnostic_prefix(),
            &mut stdout,
            &mut stderr,
        )?;
        if matches!(
            builtin,
            crate::builtins::complete::CompletionBuiltin::Compgen
        ) {
            if let Some(varname) = compgen_array_target(&cmd.words[1..]) {
                if status == 0 {
                    let values = String::from_utf8_lossy(&stdout)
                        .lines()
                        .map(str::to_string)
                        .collect();
                    store_indexed_array(&mut self.env_vars, &varname, values);
                }
                stdout.clear();
            }
        }
        self.write_buffered_builtin_output(cmd, &stdout, &stderr)?;
        Ok(status)
    }
}

fn compgen_array_target(words: &[String]) -> Option<String> {
    let mut index = 0;
    while let Some(word) = words.get(index) {
        if word == "--" || !word.starts_with('-') || word == "-" {
            return None;
        }

        let mut chars = word[1..].char_indices().peekable();
        while let Some((offset, option)) = chars.next() {
            match option {
                'V' => {
                    let value_start = 1 + offset + option.len_utf8();
                    if value_start < word.len() {
                        return Some(word[value_start..].to_string());
                    }
                    return words.get(index + 1).cloned();
                }
                'A' | 'C' | 'F' | 'G' | 'P' | 'S' | 'W' | 'X' | 'o' => {
                    if chars.peek().is_none() {
                        index += 1;
                    }
                    break;
                }
                _ => {}
            }
        }
        index += 1;
    }

    None
}

struct WaitAnyRequest {
    operands: Vec<String>,
    assign_var: Option<String>,
}

fn wait_any_request(words: &[String]) -> Option<WaitAnyRequest> {
    let mut index = 0;
    let mut wait_any = false;
    let mut assign_var = None;
    while let Some(word) = words.get(index) {
        if word == "--" {
            index += 1;
            break;
        }
        if !word.starts_with('-') || word == "-" {
            break;
        }

        for (offset, option) in word[1..].char_indices() {
            match option {
                'n' => wait_any = true,
                'f' => {}
                'p' => {
                    let value_start = 1 + offset + option.len_utf8();
                    let name = if value_start < word.len() {
                        &word[value_start..]
                    } else {
                        index += 1;
                        words.get(index)?
                    };
                    if !is_shell_name(name) {
                        return None;
                    }
                    assign_var = Some(name.to_string());
                    if value_start < word.len() {
                        break;
                    }
                    break;
                }
                _ => return None,
            }
        }
        index += 1;
    }

    wait_any.then(|| WaitAnyRequest {
        operands: words[index..].to_vec(),
        assign_var,
    })
}

fn wait_background_operands(words: &[String]) -> Option<Vec<String>> {
    let mut index = 0;
    while let Some(word) = words.get(index) {
        if word == "--" {
            index += 1;
            break;
        }
        if !word.starts_with('-') || word == "-" {
            break;
        }

        let mut chars = word[1..].chars().peekable();
        while let Some(option) = chars.next() {
            match option {
                'f' => {}
                'n' | 'p' => return None,
                _ => return None,
            }
        }
        index += 1;
    }

    Some(words[index..].to_vec())
}

fn write_wait_operand_error<E>(
    operand: &str,
    diagnostic_prefix: &str,
    stderr: &mut E,
) -> Result<i32, ExecuteError>
where
    E: Write,
{
    if operand.starts_with('%') {
        writeln!(stderr, "{diagnostic_prefix}wait: {operand}: no such job")?;
        return Ok(127);
    }

    if operand.chars().all(|ch| ch.is_ascii_digit()) {
        writeln!(
            stderr,
            "{diagnostic_prefix}wait: pid {operand} is not a child of this shell"
        )?;
        return Ok(127);
    }

    writeln!(
        stderr,
        "{diagnostic_prefix}wait: `{operand}': not a pid or valid job spec"
    )?;
    Ok(1)
}
