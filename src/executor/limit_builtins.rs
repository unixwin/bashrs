use super::*;

impl Executor {
    pub(in crate::executor) fn execute_kill(
        &mut self,
        cmd: &CommandNode,
    ) -> Result<i32, ExecuteError> {
        if let Some(status) = self.execute_tracked_background_kill(cmd)? {
            return Ok(status);
        }

        if let Some(redirect) = &cmd.redirect_out {
            let target = self.expand_word(&redirect.target);
            let mut file = File::create(shell_path_to_windows(&target, &self.env_vars))?;
            return Ok(crate::builtins::kill::execute_with_io(
                &cmd.words[1..],
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
            return Ok(crate::builtins::kill::execute_with_io(
                &cmd.words[1..],
                &mut file,
                &mut std::io::stderr().lock(),
            )?);
        }

        if let Some(redirect) = &cmd.redirect_err {
            let target = self.expand_word(&redirect.target);
            if is_null_device(&target) {
                return Ok(crate::builtins::kill::execute_with_io(
                    &cmd.words[1..],
                    &mut std::io::stdout().lock(),
                    &mut std::io::sink(),
                )?);
            }
            let mut file = File::create(shell_path_to_windows(&target, &self.env_vars))?;
            return Ok(crate::builtins::kill::execute_with_io(
                &cmd.words[1..],
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
            return Ok(crate::builtins::kill::execute_with_io(
                &cmd.words[1..],
                &mut std::io::stdout().lock(),
                &mut file,
            )?);
        }

        Ok(crate::builtins::kill::execute(&cmd.words[1..])?)
    }

    fn execute_tracked_background_kill(
        &mut self,
        cmd: &CommandNode,
    ) -> Result<Option<i32>, ExecuteError> {
        let Some(request) = kill_request(&cmd.words[1..]) else {
            return Ok(None);
        };
        if request.operands.is_empty() {
            return Ok(None);
        }

        let should_handle = request.operands.iter().any(|operand| {
            operand.starts_with('%')
                || operand
                    .parse::<u32>()
                    .ok()
                    .is_some_and(|pid| self.background_children.contains_key(&pid))
        });
        if !should_handle {
            if !request.check_only {
                return Ok(None);
            }

            let mut stderr = Vec::new();
            let mut status = 0;
            for operand in request.operands {
                let Some(pid) = operand.parse::<u32>().ok() else {
                    continue;
                };
                if !process_exists(pid) {
                    writeln!(
                        stderr,
                        "{}kill: ({pid}) - No such process",
                        self.diagnostic_prefix()
                    )?;
                    status = 1;
                }
            }
            self.write_buffered_builtin_output(cmd, &[], &stderr)?;
            return Ok(Some(status));
        }

        let mut stderr = Vec::new();
        let mut status = 0;
        for operand in request.operands {
            let Some(pid) = self.resolve_background_job(&operand) else {
                writeln!(
                    stderr,
                    "{}kill: {operand}: no such job",
                    self.diagnostic_prefix()
                )?;
                status = 1;
                continue;
            };

            if request.check_only {
                continue;
            }

            if let Some(mut child) = self.background_children.remove(&pid) {
                if child.kill().is_err() {
                    status = 1;
                }
                let _ = child.wait();
            }
            self.background_jobs.remove(&pid);
            self.background_job_order.retain(|job_pid| *job_pid != pid);
            self.coproc_stdin_writers.remove(&pid);
            self.coproc_stdout_readers.remove(&pid);
        }

        self.write_buffered_builtin_output(cmd, &[], &stderr)?;
        Ok(Some(status))
    }

    pub(in crate::executor) fn execute_ulimit(
        &mut self,
        cmd: &CommandNode,
    ) -> Result<i32, ExecuteError> {
        if let Some(redirect) = &cmd.redirect_out {
            let target = self.expand_word(&redirect.target);
            let mut file = File::create(shell_path_to_windows(&target, &self.env_vars))?;
            return Ok(crate::builtins::ulimit::execute_with_io(
                &cmd.words[1..],
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
            return Ok(crate::builtins::ulimit::execute_with_io(
                &cmd.words[1..],
                &mut self.env_vars,
                &mut file,
                &mut std::io::stderr().lock(),
            )?);
        }

        if let Some(redirect) = &cmd.redirect_err {
            let target = self.expand_word(&redirect.target);
            if is_null_device(&target) {
                return Ok(crate::builtins::ulimit::execute_with_io(
                    &cmd.words[1..],
                    &mut self.env_vars,
                    &mut std::io::stdout().lock(),
                    &mut std::io::sink(),
                )?);
            }
            let mut file = File::create(shell_path_to_windows(&target, &self.env_vars))?;
            return Ok(crate::builtins::ulimit::execute_with_io(
                &cmd.words[1..],
                &mut self.env_vars,
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
            return Ok(crate::builtins::ulimit::execute_with_io(
                &cmd.words[1..],
                &mut self.env_vars,
                &mut std::io::stdout().lock(),
                &mut file,
            )?);
        }

        Ok(crate::builtins::ulimit::execute(
            &cmd.words[1..],
            &mut self.env_vars,
        )?)
    }
}

#[cfg(unix)]
fn process_exists(pid: u32) -> bool {
    // Signal 0 performs the existence/permission check without delivering a
    // signal.  A negative result with EPERM still means the process exists.
    let result = unsafe { libc::kill(pid as libc::pid_t, 0) };
    result == 0 || std::io::Error::last_os_error().raw_os_error() == Some(libc::EPERM)
}

#[cfg(windows)]
fn process_exists(pid: u32) -> bool {
    // `tasklist` is available on supported Windows versions and avoids
    // depending on a third-party process enumeration crate.  CSV output is
    // stable across localized system messages.
    let Ok(output) = std::process::Command::new("tasklist")
        .args(["/FI", &format!("PID eq {pid}"), "/FO", "CSV", "/NH"])
        .output()
    else {
        return false;
    };
    let needle = format!(",\"{pid}\",");
    String::from_utf8_lossy(&output.stdout).contains(&needle)
}

#[cfg(not(any(unix, windows)))]
fn process_exists(_pid: u32) -> bool {
    false
}

struct KillRequest {
    operands: Vec<String>,
    check_only: bool,
}

fn kill_request(words: &[String]) -> Option<KillRequest> {
    let mut index = 0;
    let mut check_only = false;
    while let Some(word) = words.get(index) {
        if word == "--" {
            index += 1;
            break;
        }
        if word == "-l" || word == "--list" {
            return None;
        }
        if word == "-s" || word == "-n" {
            check_only |= words.get(index + 1).is_some_and(|signal| signal == "0");
            index += 2;
            continue;
        }
        if word.starts_with('-') && word != "-" {
            check_only |= word == "-0";
            index += 1;
            continue;
        }
        break;
    }

    Some(KillRequest {
        operands: words[index..].to_vec(),
        check_only,
    })
}
