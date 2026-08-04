use super::*;

impl Executor {
    /// Execute an AST
    pub fn execute_command(&mut self, cmd: &CommandNode) -> Result<(), ExecuteError> {
        // While a DEBUG trap action runs, LINENO must keep pointing at the
        // about-to-run command that triggered the trap (dbg-support2.tests
        // `print_trap $LINENO`); the action's own commands must not move it.
        if !self.debug_trap_running {
            self.set_current_line(cmd);
        }
        self.set_current_command(cmd);
        self.report_command_heredoc_errors(cmd)?;

        if let Some(result) = self.execute_initial_command_node(cmd) {
            // Compound commands run through execute_initial_command_node and
            // bypass the errexit check in execute_materialized_command. A
            // failing subshell must still honor `set -e`: `(exit 17)` exits
            // the script (set-e1.sub). &&/||/! contexts already suppressed
            // errexit at the ast_exec call site.
            if cmd.subshell_command.is_some()
                && result.is_ok()
                && self.errexit_enabled()
                && self.errexit_is_active()
                && self.exit_code != 0
            {
                return Err(ExecuteError::ExitCode(self.exit_code));
            }
            return result;
        }

        if cmd.words.is_empty() {
            return self.execute_empty_words_command(cmd);
        }

        self.validate_command_parameter_expansions(cmd)?;

        if self.execute_parser_level_alias(cmd)? {
            return Ok(());
        }

        let expanded = self.expand_command_words(cmd)?;
        let original_raws: Vec<Option<&str>> = cmd
            .word_metadata
            .iter()
            .map(|metadata| Some(metadata.raw.as_str()))
            .collect();
        let cmd =
            self.apply_alias_expansion_after_word_expansion(expanded, &original_raws);

        if self.execute_alias_expanded_syntax(&cmd)? {
            return Ok(());
        }

        if let Some(result) = self.execute_function_command_invocation(&cmd) {
            return result;
        }

        if self.execute_assignment_or_comment_command(&cmd) {
            return Ok(());
        }

        if !command_needs_process_substitution_materialization(&cmd) {
            return self.execute_materialized_command(&cmd, ProcessSubstitutionFiles::default());
        }

        let (materialized_cmd, process_substitution_files) =
            self.command_with_process_substitution_files(&cmd)?;
        self.execute_materialized_command(&materialized_cmd, process_substitution_files)
    }
}
