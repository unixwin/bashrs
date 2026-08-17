use super::*;

impl Executor {
    pub(in crate::executor) fn execute_sudo(
        &mut self,
        cmd: &CommandNode,
    ) -> Result<i32, ExecuteError> {
        let args = cmd.words[1..]
            .iter()
            .map(|word| self.expand_word(word))
            .collect::<Vec<_>>();

        let invocation = match crate::builtins::sudo::parse(&args) {
            Ok(crate::builtins::sudo::SudoAction::Complete(status)) => {
                let mut stdout = Vec::new();
                crate::builtins::sudo::print_help_with_io(&mut stdout)?;
                self.write_buffered_builtin_output(cmd, &stdout, &[])?;
                return Ok(status);
            }
            Ok(crate::builtins::sudo::SudoAction::Run(invocation)) => invocation,
            Err(message) => {
                let mut stderr = Vec::new();
                writeln!(
                    &mut stderr,
                    "{}sudo: {message}",
                    self.diagnostic_prefix()
                )?;
                writeln!(
                    &mut stderr,
                    "sudo: usage: sudo [-E] [--inline|--new-window] [--] command [arg ...]"
                )?;
                self.write_buffered_builtin_output(cmd, &[], &stderr)?;
                return Ok(2);
            }
        };

        let Some(handler) = self.elevation_handler.as_mut() else {
            let mut stderr = Vec::new();
            writeln!(
                &mut stderr,
                "{}sudo: elevation handler is not configured",
                self.diagnostic_prefix()
            )?;
            self.write_buffered_builtin_output(cmd, &[], &stderr)?;
            return Ok(1);
        };

        let environment = if invocation.preserve_environment {
            self.env_vars.clone()
        } else {
            self.child_shell_environment()
        };
        let current_dir = env::current_dir().unwrap_or_else(|_| PathBuf::from("."));
        let request = ElevationRequest {
            command: invocation.command,
            environment,
            current_dir,
            preserve_environment: invocation.preserve_environment,
            mode: invocation.mode,
        };

        match (handler.0)(request) {
            Ok(output) => {
                self.write_buffered_builtin_output(cmd, &output.stdout, &output.stderr)?;
                Ok(output.status)
            }
            Err(message) => {
                let mut stderr = Vec::new();
                writeln!(
                    &mut stderr,
                    "{}sudo: {message}",
                    self.diagnostic_prefix()
                )?;
                self.write_buffered_builtin_output(cmd, &[], &stderr)?;
                Ok(1)
            }
        }
    }
}
