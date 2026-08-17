use super::*;

impl Executor {
    pub(in crate::executor) fn execute_hash(
        &mut self,
        cmd: &CommandNode,
    ) -> Result<i32, ExecuteError> {
        // TODO(redir.c/builtins/hash.def): Redirections are command-level in
        // Bash. This covers `hash -t cat 2>/dev/null` from builtins9.sub.
        let mut stdout = Vec::new();
        let mut stderr = Vec::new();
        let status = crate::builtins::hash::execute_with_io(
            &cmd.words[1..],
            &mut self.env_vars,
            &mut stdout,
            &mut stderr,
        )?;
        self.write_buffered_builtin_output(cmd, &stdout, &stderr)?;
        Ok(status)
    }

    pub(in crate::executor) fn execute_shopt(
        &mut self,
        cmd: &CommandNode,
    ) -> Result<i32, ExecuteError> {
        let mut stdout = Vec::new();
        let mut stderr = Vec::new();
        let status = crate::builtins::shopt::execute_with_io(
            &cmd.words[1..],
            &mut self.env_vars,
            &mut stdout,
            &mut stderr,
        )?;
        self.write_buffered_builtin_output(cmd, &stdout, &stderr)?;
        Ok(status)
    }

    pub(in crate::executor) fn execute_zsh_option_builtin(
        &mut self,
        cmd: &CommandNode,
        enable: bool,
    ) -> Result<i32, ExecuteError> {
        let mut stdout = Vec::new();
        let mut stderr = Vec::new();
        let command_name = if enable { "setopt" } else { "unsetopt" };
        let status = crate::builtins::zsh_options::execute_with_io(
            command_name,
            enable,
            &cmd.words[1..],
            &mut self.env_vars,
            &mut stdout,
            &mut stderr,
        )?;
        self.write_buffered_builtin_output(cmd, &stdout, &stderr)?;
        Ok(status)
    }

    pub(in crate::executor) fn execute_umask(
        &mut self,
        cmd: &CommandNode,
    ) -> Result<i32, ExecuteError> {
        let mut stdout = Vec::new();
        let mut stderr = Vec::new();
        let status = crate::builtins::umask::execute_with_io(
            &cmd.words[1..],
            &mut self.env_vars,
            &mut stdout,
            &mut stderr,
        )?;
        self.write_buffered_builtin_output(cmd, &stdout, &stderr)?;
        Ok(status)
    }
}
