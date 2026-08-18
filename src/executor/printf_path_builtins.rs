use super::*;

impl Executor {
    pub(in crate::executor) fn execute_printf(
        &mut self,
        cmd: &CommandNode,
    ) -> Result<i32, ExecuteError> {
        let mut stdout = Vec::new();
        let mut stderr = Vec::new();
        let status = crate::builtins::printf::execute_with_io_and_store(
            cmd.words[1..].iter().map(String::as_str),
            &mut self.env_vars,
            Some(&mut self.shell_state.variables),
            &mut stdout,
            &mut stderr,
        )?;
        self.write_buffered_builtin_output(cmd, &stdout, &stderr)?;
        Ok(status)
    }

    pub(in crate::executor) fn execute_exit(
        &mut self,
        cmd: &CommandNode,
    ) -> Result<crate::builtins::exit::ExitAction, ExecuteError> {
        let mut stdout = Vec::new();
        let mut stderr = Vec::new();
        let action = crate::builtins::exit::execute_with_io(
            cmd.words[1..].iter().map(String::as_str),
            self.exit_code,
            &mut stdout,
            &mut stderr,
        )?;
        self.write_buffered_builtin_output(cmd, &stdout, &stderr)?;
        Ok(action)
    }

    pub(in crate::executor) fn execute_logout(
        &mut self,
        cmd: &CommandNode,
    ) -> Result<i32, ExecuteError> {
        let mut stderr = Vec::new();
        let status =
            crate::builtins::logout::execute_with_io(&self.diagnostic_prefix(), &mut stderr)?;
        self.write_buffered_builtin_output(cmd, &[], &stderr)?;
        Ok(status)
    }

    pub(in crate::executor) fn try_execute_dirname_fast_path(
        &mut self,
        cmd: &CommandNode,
    ) -> Option<i32> {
        let operands = simple_path_tool_operands(&cmd.words[1..])?;
        if operands.is_empty() {
            return None;
        }

        let mut stdout = Vec::new();
        for operand in operands {
            let path = self.expand_word(operand);
            stdout.extend_from_slice(dirname_value(&path).as_bytes());
            stdout.push(b'\n');
        }
        Some(self.write_path_tool_output(cmd, &stdout))
    }

    pub(in crate::executor) fn try_execute_basename_fast_path(
        &mut self,
        cmd: &CommandNode,
    ) -> Option<i32> {
        let operands = simple_path_tool_operands(&cmd.words[1..])?;
        if operands.is_empty() || operands.len() > 2 {
            return None;
        }

        let name = self.expand_word(operands[0]);
        let mut value = basename_value(&name);
        if let Some(suffix) = operands.get(1) {
            let suffix = self.expand_word(suffix);
            value = strip_basename_suffix(&value, &suffix);
        }

        let mut stdout = Vec::new();
        stdout.extend_from_slice(value.as_bytes());
        stdout.push(b'\n');
        Some(self.write_path_tool_output(cmd, &stdout))
    }

    fn write_path_tool_output(&mut self, cmd: &CommandNode, stdout: &[u8]) -> i32 {
        if self
            .write_buffered_builtin_output(cmd, stdout, &[])
            .is_err()
        {
            return 1;
        }
        0
    }

    pub(in crate::executor) fn execute_cd(
        &mut self,
        cmd: &CommandNode,
    ) -> Result<i32, ExecuteError> {
        if let Some(redirect) = &cmd.redirect_out {
            let target = self.expand_word(&redirect.target);
            if is_null_device(&target) {
                return Ok(crate::builtins::cd::execute_with_io(
                    cmd.words[1..].iter().map(String::as_str),
                    &mut self.env_vars,
                    &mut std::io::sink(),
                    &mut std::io::stderr().lock(),
                )?);
            }
            let mut file = File::create(shell_path_to_windows(&target, &self.env_vars))?;
            return Ok(crate::builtins::cd::execute_with_io(
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
            return Ok(crate::builtins::cd::execute_with_io(
                cmd.words[1..].iter().map(String::as_str),
                &mut self.env_vars,
                &mut file,
                &mut std::io::stderr().lock(),
            )?);
        }

        if let Some(redirect) = &cmd.redirect_err {
            let target = self.expand_word(&redirect.target);
            if is_null_device(&target) {
                return Ok(crate::builtins::cd::execute_with_io(
                    cmd.words[1..].iter().map(String::as_str),
                    &mut self.env_vars,
                    &mut std::io::stdout().lock(),
                    &mut std::io::sink(),
                )?);
            }
            let mut file = File::create(shell_path_to_windows(&target, &self.env_vars))?;
            return Ok(crate::builtins::cd::execute_with_io(
                cmd.words[1..].iter().map(String::as_str),
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
            return Ok(crate::builtins::cd::execute_with_io(
                cmd.words[1..].iter().map(String::as_str),
                &mut self.env_vars,
                &mut std::io::stdout().lock(),
                &mut file,
            )?);
        }

        Ok(crate::builtins::cd::execute(
            &cmd.words[1..],
            &mut self.env_vars,
        )?)
    }
}

fn simple_path_tool_operands(args: &[String]) -> Option<Vec<&str>> {
    let mut operands = Vec::new();
    let mut parse_options = true;
    for arg in args {
        if parse_options && arg == "--" {
            parse_options = false;
            continue;
        }
        if parse_options && arg.starts_with('-') && arg.len() > 1 {
            return None;
        }
        operands.push(arg.as_str());
    }
    Some(operands)
}

fn basename_value(path: &str) -> String {
    let normalized = path.replace('\\', "/");
    let trimmed = trim_trailing_slashes(&normalized);
    if trimmed.is_empty() {
        return "/".to_string();
    }
    trimmed
        .rsplit_once('/')
        .map(|(_, name)| name)
        .unwrap_or(&trimmed)
        .to_string()
}

fn dirname_value(path: &str) -> String {
    let normalized = path.replace('\\', "/");
    let trimmed = trim_trailing_slashes(&normalized);
    if trimmed.is_empty() {
        return "/".to_string();
    }
    let Some((dir, _)) = trimmed.rsplit_once('/') else {
        return ".".to_string();
    };
    let dir = trim_trailing_slashes(dir);
    if dir.is_empty() {
        "/".to_string()
    } else {
        dir
    }
}

fn trim_trailing_slashes(value: &str) -> String {
    let trimmed = value.trim_end_matches('/');
    if trimmed.is_empty() && value.contains('/') {
        String::new()
    } else {
        trimmed.to_string()
    }
}

fn strip_basename_suffix(name: &str, suffix: &str) -> String {
    if suffix.len() < name.len() && name.ends_with(suffix) {
        name[..name.len() - suffix.len()].to_string()
    } else {
        name.to_string()
    }
}
