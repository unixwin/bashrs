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

        if cmd.assignments.contains_key("__RUBASH_PARSE_ERROR__") {
            let message = cmd
                .assignments
                .get("__RUBASH_PARSE_ERROR__")
                .map(String::as_str)
                .unwrap_or("unexpected token");
            eprintln!("{}syntax error near {message}", self.diagnostic_prefix(),);
            self.exit_code = 2;
            return Err(ExecuteError::ExitCode(2));
        }

        if cmd
            .word_metadata
            .iter()
            .any(|metadata| crate::lexer::has_unclosed_command_substitution(&metadata.raw))
            || cmd
                .assignments
                .values()
                .any(|value| crate::lexer::has_unclosed_command_substitution(value))
        {
            eprintln!(
                "{}syntax error: unexpected EOF while looking for matching `)'",
                self.diagnostic_prefix()
            );
            self.exit_code = 2;
            return Err(ExecuteError::ExitCode(2));
        }

        if cmd
            .word_metadata
            .iter()
            .any(|metadata| unterminated_extglob(&metadata.raw))
        {
            eprintln!(
                "{}syntax error near unexpected token `('",
                self.diagnostic_prefix()
            );
            self.exit_code = 2;
            return Err(ExecuteError::ExitCode(2));
        }

        // Bash must parse extglob syntax while the extglob option is
        // enabled.  A pathname such as `@(name)` in a simple command is
        // therefore a syntax error when the option is off; treating it as a
        // literal silently accepts malformed scripts.  Conditional RHS
        // patterns are handled separately and intentionally remain eligible
        // for Bash's conditional-pattern semantics.
        if !crate::builtins::shopt::option_enabled(&self.env_vars, "extglob")
            && !cmd.extglob_patterns.is_empty()
            && cmd.conditional_command.is_none()
            && cmd.case_command.is_none()
        {
            eprintln!(
                "{}syntax error near unexpected token `('",
                self.diagnostic_prefix()
            );
            self.exit_code = 2;
            return Err(ExecuteError::ExitCode(2));
        }

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
        if self.last_command_substitution_status.get() == Some(2) {
            eprintln!(
                "{}syntax error in command substitution",
                self.diagnostic_prefix()
            );
            self.exit_code = 2;
            self.last_command_substitution_status.set(None);
            return Err(ExecuteError::ExitCode(2));
        }
        let original_raws: Vec<Option<&str>> = cmd
            .word_metadata
            .iter()
            .map(|metadata| Some(metadata.raw.as_str()))
            .collect();
        let cmd = self.apply_alias_expansion_after_word_expansion(expanded, &original_raws);

        // Arithmetic expansion errors are reported during word expansion.
        // The failing command itself must not be dispatched (Bash returns 1),
        // while the AST-level marker remains set so the command-list walker
        // can apply Bash's follow-up command suppression semantics.
        if self.arithmetic_expansion_error.get() {
            self.arithmetic_expansion_error.set(false);
            self.exit_code = 1;
            return Ok(());
        }

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

fn unterminated_extglob(raw: &str) -> bool {
    let chars = raw.chars().collect::<Vec<_>>();
    let mut extglob_depth = 0usize;
    let mut quote = None;
    let mut index = 0;
    while index < chars.len() {
        let ch = chars[index];
        if let Some(active) = quote {
            if ch == active {
                quote = None;
            }
            index += 1;
            continue;
        }
        if ch == '\\' {
            index += 2;
            continue;
        }
        if ch == '\'' || ch == '"' {
            quote = Some(ch);
            index += 1;
            continue;
        }
        if matches!(ch, '@' | '*' | '+' | '?' | '!') && chars.get(index + 1) == Some(&'(') {
            extglob_depth += 1;
            index += 2;
            continue;
        }
        if ch == ')' && extglob_depth > 0 {
            extglob_depth -= 1;
        }
        index += 1;
    }
    extglob_depth > 0
}
