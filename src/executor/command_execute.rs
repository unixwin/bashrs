use super::*;

impl Executor {
    /// Execute an AST
    pub fn execute_command(&mut self, cmd: &CommandNode) -> Result<(), ExecuteError> {
        // Set the source line for every command, including commands inside a
        // DEBUG trap function. The trap action expands its call-site `$LINENO`
        // before entering that function, while the function body must see its
        // own source line (dbg-support2.tests).
        if self.debug_trap_running && self.function_depth > 0 {
            if let Some(line) = self.debug_trap_function_line {
                self.env_vars
                    .insert("__RUBASH_CURRENT_LINE".to_string(), line.to_string());
            } else {
                self.set_current_line(cmd);
            }
        } else {
            self.set_current_line(cmd);
        }
        self.set_current_command(cmd);
        self.report_command_heredoc_errors(cmd)?;

        if cmd.assignments.contains_key("__RUBASH_PARSE_ERROR__") {
            self.mark_parse_error();
            if let Some(source) = cmd.assignments.get("__RUBASH_PARSE_SOURCE__") {
                if let Some(reparsed) = self.reparse_reserved_word_aliases(source) {
                    let tokens = crate::lexer::tokenize(&reparsed);
                    let ast = crate::parser::parse(&tokens);
                    return self.execute_ast(&ast);
                }
            }
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
            self.mark_parse_error();
            eprintln!(
                "{}syntax error: unexpected EOF while looking for matching `)'",
                self.diagnostic_prefix()
            );
            self.exit_code = 2;
            return Err(ExecuteError::ExitCode(2));
        }

        if cmd.function_command.is_none()
            && cmd
                .word_metadata
                .iter()
                .any(|metadata| unterminated_extglob(&metadata.raw))
        {
            self.mark_parse_error();
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
            self.mark_parse_error();
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

        if let Some((name, message, status)) = self.parameter_heredoc_expansion_error(cmd) {
            let mut stderr = Vec::new();
            writeln!(
                &mut stderr,
                "{}{}: {}",
                self.diagnostic_prefix(),
                name,
                message
            )?;
            self.write_default_stderr(&stderr)?;
            self.exit_code = status;
            return Ok(());
        }

        self.validate_command_parameter_expansions(cmd)?;

        if self.execute_parser_level_alias(cmd)? {
            return Ok(());
        }

        let expanded = self.expand_command_words(cmd)?;
        if self.last_command_substitution_status.get() == Some(2) {
            self.mark_parse_error();
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
        let alias_expanded =
            self.apply_alias_expansion_after_word_expansion(expanded, &original_raws);
        if alias_expanded.words.is_empty() {
            if !cmd.assignments.is_empty() {
                return self.execute_empty_words_command(cmd);
            }
            if let Some(status) = self.last_command_substitution_status.get() {
                self.exit_code = status;
                self.last_command_substitution_status.set(None);
            }
            if self.errexit_enabled() && self.errexit_is_active() && self.exit_code != 0 {
                return Err(ExecuteError::ExitCode(self.exit_code));
            }
            return Ok(());
        }
        let cmd = alias_expanded;
        // Arithmetic expansion errors are fatal expansion errors in Bash. The
        // failing command is not dispatched and the surrounding command list
        // must stop, including when it is followed by `||` or `&&`.
        if self.arithmetic_expansion_error.get() {
            self.arithmetic_expansion_error.set(false);
            let status = if self
                .env_vars
                .remove("__RUBASH_ARITH_NOUNSET_ERROR")
                .is_some()
            {
                127
            } else {
                1
            };
            self.exit_code = status;
            let script_mode_nonfatal = self.env_vars.contains_key("__RUBASH_SCRIPT_NAME")
                && self.subshell_depth.get() == 0
                && (!self.errexit_enabled() || !self.errexit_is_active());
            if self.arithmetic_nonfatal_error.replace(false) || script_mode_nonfatal {
                return Ok(());
            }
            return Err(ExecuteError::ExitCode(status));
        }

        if self.execute_alias_expanded_syntax(&cmd)? {
            return Ok(());
        }

        // Unquoted command substitutions can disappear during word
        // expansion. A command that started as `name=$(...)` may therefore
        // become assignment-only and must still apply the assignment and its
        // redirections.
        if cmd.words.is_empty() {
            return self.execute_empty_words_command(&cmd);
        }

        if let Some(result) = self.execute_function_command_invocation(&cmd) {
            return result;
        }

        if self.execute_assignment_or_comment_command(&cmd) {
            return Ok(());
        }

        // `exec {fd}...` mutates the shell's persistent descriptor table.
        // Do not materialize its input redirect through the external-command
        // path: that would consume the source virtual fd before exec can
        // duplicate or move it.
        if is_dynamic_fd_exec_command(&cmd)
            || !command_needs_process_substitution_materialization(&cmd)
        {
            return self.execute_materialized_command(&cmd, ProcessSubstitutionFiles::default());
        }

        let (materialized_cmd, process_substitution_files) =
            self.command_with_process_substitution_files(&cmd)?;
        self.execute_materialized_command(&materialized_cmd, process_substitution_files)
    }
}

fn is_dynamic_fd_exec_command(cmd: &CommandNode) -> bool {
    cmd.words.first().map(String::as_str) == Some("exec")
        && cmd.words.get(1).is_some_and(|word| {
            let Some(name) = word
                .strip_prefix('{')
                .and_then(|word| word.strip_suffix('}'))
            else {
                return false;
            };
            !name.is_empty()
                && name
                    .chars()
                    .all(|ch| ch == '_' || ch.is_ascii_alphanumeric())
        })
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
    if extglob_depth > 0 {
        return true;
    }
    false
}
