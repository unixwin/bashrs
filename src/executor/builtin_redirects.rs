use super::*;

impl Executor {
    pub(in crate::executor) fn write_builtin_not_found(
        &mut self,
        cmd: &CommandNode,
        name: &str,
    ) -> Result<(), ExecuteError> {
        let mut stderr = Vec::new();
        writeln!(
            &mut stderr,
            "{}builtin: {name}: not a shell builtin",
            self.diagnostic_prefix()
        )?;
        self.write_buffered_builtin_output(cmd, &[], &stderr)
    }

    pub(in crate::executor) fn apply_no_output_builtin_redirects(
        &mut self,
        cmd: &CommandNode,
    ) -> Result<(), ExecuteError> {
        self.apply_no_output_builtin_redirects_with_status(cmd)
            .map(|_| ())
    }

    pub(in crate::executor) fn apply_no_output_builtin_redirects_with_status(
        &mut self,
        cmd: &CommandNode,
    ) -> Result<bool, ExecuteError> {
        let mut redirect_failed = false;
        let auto_close = cmd.words.first().map(String::as_str) != Some("exec");
        for redirect in &cmd.redirects {
            if redirect.fd_var.is_some() {
                match self.execute_dynamic_fd_var_redirect(redirect, auto_close) {
                    Ok(_) => {}
                    Err(ExecuteError::IoError(error)) => {
                        let mut stderr = Vec::new();
                        writeln!(
                            &mut stderr,
                            "{}{}",
                            self.diagnostic_prefix(),
                            crate::posix_errors::message(&error)
                        )?;
                        self.write_default_stderr(&stderr)?;
                        self.exit_code = 1;
                        redirect_failed = true;
                    }
                    Err(error) => return Err(error),
                }
            }
        }

        if let Some(redirect) = &cmd.redirect_in {
            let target = self.expand_word(&redirect.target);
            if redirect.fd_var.is_some() {
            } else if is_closed_redirect_target(&target) {
            } else if redirect.append {
                OpenOptions::new()
                    .create(true)
                    .read(true)
                    .write(true)
                    .open(shell_path_to_windows(&target, &self.env_vars))?;
            } else {
                self.open_input_redirect(&target)?;
            }
        }

        if let Some(redirect) = &cmd.redirect_out {
            let target = self.expand_word(&redirect.target);
            if redirect.fd_var.is_some() {
            } else if !is_closed_redirect_target(&target) && redirect_target_fd(&target).is_none() {
                self.create_redirect_output(&target, redirect.clobber)?;
            }
        }

        if let Some(redirect) = &cmd.append {
            let target = self.expand_word(&redirect.target);
            if redirect.fd_var.is_some() {
            } else if !is_closed_redirect_target(&target) && redirect_target_fd(&target).is_none() {
                self.open_output_fd_append(&target).or_else(|_| {
                    if is_null_device(&target) {
                        self.create_redirect_output(&target, true)
                    } else {
                        OpenOptions::new()
                            .create(true)
                            .append(true)
                            .open(shell_path_to_windows(&target, &self.env_vars))
                    }
                })?;
            }
        }

        if let Some(redirect) = &cmd.redirect_err {
            let target = self.expand_word(&redirect.target);
            if redirect.fd_var.is_some() {
            } else if !is_closed_redirect_target(&target)
                && !is_null_device(&target)
                && redirect_target_fd(&target).is_none()
            {
                self.create_redirect_output(&target, redirect.clobber)?;
            }
        }

        if let Some(redirect) = &cmd.redirect_err_append {
            let target = self.expand_word(&redirect.target);
            if redirect.fd_var.is_some() {
            } else if !is_closed_redirect_target(&target) && redirect_target_fd(&target).is_none() {
                OpenOptions::new()
                    .create(true)
                    .append(true)
                    .open(shell_path_to_windows(&target, &self.env_vars))?;
            }
        }

        Ok(redirect_failed)
    }
}
