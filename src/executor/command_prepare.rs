use super::glob::{pathname_expand_word, PathnameExpansion};
use super::*;

impl Executor {
    pub(in crate::executor) fn report_command_heredoc_errors(
        &mut self,
        cmd: &CommandNode,
    ) -> Result<(), ExecuteError> {
        if cmd.subshell && command_has_unterminated_heredoc(cmd) {
            self.report_unterminated_subshell_heredoc(cmd);
            self.exit_code = 2;
            return Err(ExecuteError::ExitCode(2));
        }
        if command_has_unterminated_heredoc(cmd) {
            self.report_unterminated_heredoc(cmd);
        }
        Ok(())
    }

    pub(in crate::executor) fn execute_initial_command_node(
        &mut self,
        cmd: &CommandNode,
    ) -> Option<Result<(), ExecuteError>> {
        if command_is_time_prefixed_compound(cmd) {
            return Some(self.execute_time_prefixed_compound_command(cmd));
        }
        if let Some(for_command) = &cmd.for_command {
            return Some(self.execute_for_command_with_redirects(for_command, cmd));
        }
        if let Some(if_command) = &cmd.if_command {
            return Some(self.execute_if_command_with_redirects(cmd, if_command));
        }
        if let Some(loop_command) = &cmd.loop_command {
            return Some(self.execute_loop_command_with_redirects(cmd, loop_command));
        }
        if let Some(subshell_command) = &cmd.subshell_command {
            return Some(self.execute_subshell_command_with_redirects(cmd, subshell_command));
        }
        if let Some(select_command) = &cmd.select_command {
            return Some(self.execute_select_command(cmd, select_command));
        }
        if let Some(case_command) = &cmd.case_command {
            return Some(self.execute_case_command_with_redirects(cmd, case_command));
        }
        if let Some(coproc_cmd) = &cmd.coproc_command {
            return Some(self.execute_coproc_command(cmd, coproc_cmd));
        }
        if let Some(conditional_command) = &cmd.conditional_command {
            return Some(self.execute_conditional_command_with_redirects(cmd, conditional_command));
        }
        if let Some(function_command) = &cmd.function_command {
            return Some(self.define_function(cmd, function_command));
        }
        None
    }

    fn execute_conditional_command_with_redirects(
        &mut self,
        cmd: &CommandNode,
        conditional_command: &ConditionalCommand,
    ) -> Result<(), ExecuteError> {
        self.apply_no_output_builtin_redirects(cmd)?;
        self.exit_code = self.execute_conditional_command(conditional_command);
        Ok(())
    }

    pub(in crate::executor) fn execute_empty_words_command(
        &mut self,
        cmd: &CommandNode,
    ) -> Result<(), ExecuteError> {
        if command_has_no_effect(cmd) {
            return Ok(());
        }
        if let Some((name, message)) = self.parameter_assignment_error(cmd) {
            eprintln!("{}{}: {}", self.diagnostic_prefix(), name, message);
            self.exit_code = 1;
            return Err(ExecuteError::ExitCode(1));
        }
        if let Some((name, message, status)) = self.parameter_expansion_error(cmd) {
            eprintln!("{}{}: {}", self.diagnostic_prefix(), name, message);
            self.exit_code = status;
            return Err(ExecuteError::ExitCode(status));
        }
        let mut status = 0;
        for (name, value) in &cmd.assignments {
            let (expanded_value, substitution_status) =
                self.expand_assignment_value_with_status(value);
            if let Some(substitution_status) = substitution_status {
                status = substitution_status;
            }
            if !self.apply_shell_assignment(name, expanded_value) {
                status = 1;
            }
        }
        self.apply_no_output_builtin_redirects(cmd)?;
        self.exit_code = status;
        if self.errexit_enabled() && self.errexit_is_active() && self.exit_code != 0 {
            return Err(ExecuteError::ExitCode(self.exit_code));
        }
        Ok(())
    }

    pub(in crate::executor) fn validate_command_parameter_expansions(
        &mut self,
        cmd: &CommandNode,
    ) -> Result<(), ExecuteError> {
        if let Some((name, message)) = self.parameter_assignment_error(cmd) {
            eprintln!("{}{}: {}", self.diagnostic_prefix(), name, message);
            self.exit_code = 1;
            return Err(ExecuteError::ExitCode(1));
        }
        self.apply_parameter_assignment_expansions(cmd);
        if let Some((name, message, status)) = self.parameter_expansion_error(cmd) {
            eprintln!("{}{}: {}", self.diagnostic_prefix(), name, message);
            self.exit_code = status;
            return Err(ExecuteError::ExitCode(status));
        }
        Ok(())
    }

    pub(in crate::executor) fn expand_command_words(
        &mut self,
        cmd: &CommandNode,
    ) -> Result<CommandNode, ExecuteError> {
        let preserve_word_metadata = cmd.conditional_command.is_some()
            || cmd.words.first().is_some_and(|word| word == "[[")
            || !cmd.process_substitutions.is_empty()
            || cmd.word_metadata.iter().any(|metadata| {
                !metadata.process_substitutions.is_empty()
                    || raw_word_contains_process_substitution(Some(&metadata.raw))
            });
        let mut variable_expanded = CommandNode {
            words: Vec::new(),
            word_metadata: if preserve_word_metadata {
                cmd.word_metadata.clone()
            } else {
                Vec::new()
            },
            word_kinds: Vec::new(),
            assignments: cmd.assignments.clone(),
            compound_assignments: cmd.compound_assignments.clone(),
            array_element_assignments: cmd.array_element_assignments.clone(),
            process_substitutions: cmd.process_substitutions.clone(),
            redirects: cmd.redirects.clone(),
            redirect_in: cmd.redirect_in.clone(),
            redirect_out: cmd.redirect_out.clone(),
            append: cmd.append.clone(),
            redirect_err: cmd.redirect_err.clone(),
            redirect_err_append: cmd.redirect_err_append.clone(),
            heredoc: cmd.heredoc.clone(),
            heredoc_delimiter: cmd.heredoc_delimiter.clone(),
            heredoc_redirects: cmd.heredoc_redirects.clone(),
            here_string: cmd.here_string.clone(),
            pipe: cmd.pipe,
            background: cmd.background,
            and_or: cmd.and_or,
            inverted: cmd.inverted,
            arithmetic_command: cmd.arithmetic_command.clone(),
            conditional_command: cmd.conditional_command.clone(),
            brace_group: cmd.brace_group.clone(),
            line: cmd.line,
            ..CommandNode::new()
        };
        let expanded_words = cmd
            .words
            .iter()
            .enumerate()
            .flat_map(|(index, word)| {
                let raw = cmd
                    .word_metadata
                    .get(index)
                    .map(|metadata| metadata.raw.as_str());
                let suppress_glob =
                    word.starts_with('\x1b') || word.starts_with('\x1d') || raw_word_is_quoted(raw);
                self.expand_command_word(cmd, index, word, raw)
                    .into_iter()
                    .map(move |word| (word, suppress_glob))
            })
            .collect::<Vec<_>>();
        variable_expanded.words = expanded_words
            .iter()
            .map(|(word, _)| word.clone())
            .collect();
        variable_expanded.word_kinds = Vec::new();

        let is_test_cmd = cmd.words.first().is_some_and(|w| w == "[[" || w == "[");
        if !is_test_cmd {
            let mut words = Vec::new();
            for (word, suppress_glob) in expanded_words {
                if suppress_glob {
                    words.push(word);
                } else {
                    match pathname_expand_word(&word, &self.env_vars) {
                        PathnameExpansion::Matches(matches) => words.extend(matches),
                        PathnameExpansion::NoMatch => words.push(word),
                        PathnameExpansion::Fail(pattern) => {
                            self.report_failglob(&pattern);
                            return Err(ExecuteError::ExitCode(1));
                        }
                    }
                }
            }
            variable_expanded.words = words;
        }
        Ok(variable_expanded)
    }

    fn expand_command_word(
        &mut self,
        cmd: &CommandNode,
        index: usize,
        word: &str,
        raw: Option<&str>,
    ) -> Vec<String> {
        if cmd
            .process_substitutions
            .iter()
            .any(|process| process.word_index == Some(index) && process.target == word)
        {
            return vec![word.to_string()];
        }
        if let Some(values) = self.braced_alternate_word_values(word) {
            return values;
        }
        if let Some(values) = self.array_at_word_values(word) {
            if word_is_unquoted_array_list_expansion(word) {
                return field_split_array_values_with_ifs(
                    values,
                    self.env_vars.get("IFS").map(String::as_str),
                );
            }
            return values;
        }
        if let Some(values) =
            self.quoted_positional_at_word_values_with_raw(word, raw, cmd.word_kinds.get(index))
        {
            if self.word_is_unquoted_positional_modified_list_expansion(word) {
                return field_split_array_values_with_ifs(
                    values,
                    self.env_vars.get("IFS").map(String::as_str),
                );
            }
            return values;
        }
        if self.is_brace_expand_enabled() && !word.contains("${") {
            let braced = expand_braces_with_optional_raw(word, raw);
            if braced.len() > 1 {
                return braced;
            }
        }
        if raw_word_is_quoted(raw) {
            if let Some(expanded) = self.expand_backtick_substitution(word) {
                return vec![expanded];
            }
        }
        let expanded = self.expand_word_mut(word);
        if expanded.is_empty() && self.removes_unquoted_null_word(cmd, index) {
            Vec::new()
        } else if raw_word_contains_process_substitution(raw)
            && expanded_word_has_process_substitution(&expanded)
        {
            vec![expanded]
        } else if let Some(values) = self.field_split_word_with_quoted_empty_suffix(raw, &expanded)
        {
            values
        } else if self.splits_unquoted_expanded_word(cmd, index, &expanded) {
            self.field_split_values(&expanded)
        } else {
            vec![expanded]
        }
    }

    fn field_split_word_with_quoted_empty_suffix(
        &self,
        raw: Option<&str>,
        expanded: &str,
    ) -> Option<Vec<String>> {
        let raw = raw?;
        if !raw_has_quoted_empty_suffix(raw) || !expanded_ends_with_ifs_separator(expanded, self) {
            return None;
        }

        let mut values = self.field_split_values(expanded);
        values.push(String::new());
        Some(values)
    }

    fn braced_alternate_word_values(&mut self, word: &str) -> Option<Vec<String>> {
        let name = word.strip_prefix("${")?.strip_suffix('}')?;
        if !braced_parameter_spans_whole_word(word) {
            return None;
        }

        let (var_name, alternate, require_non_empty) =
            if let Some((var_name, alternate)) = name.split_once(":+") {
                (var_name, alternate, true)
            } else if let Some((var_name, alternate)) = name.split_once('+') {
                (var_name, alternate, false)
            } else {
                return None;
            };

        let value = self.parameter_operator_value(var_name)?;
        if require_non_empty && value.is_empty() {
            return None;
        }

        Some(self.expand_alternate_word_fragment(alternate))
    }

    fn expand_alternate_word_fragment(&mut self, fragment: &str) -> Vec<String> {
        let source = format!("__rubash_parameter_alternate__ {fragment}");
        let tokens = crate::lexer::tokenize(&source);
        let ast = crate::parser::parse(&tokens);
        let Some(cmd) = ast.commands.first() else {
            return Vec::new();
        };

        let mut values = Vec::new();
        for index in 1..cmd.words.len() {
            let raw = cmd
                .word_metadata
                .get(index)
                .map(|metadata| metadata.raw.as_str());
            values.extend(self.expand_command_word(cmd, index, &cmd.words[index], raw));
        }
        values
    }

    pub(in crate::executor) fn apply_alias_expansion_after_word_expansion(
        &mut self,
        mut variable_expanded: CommandNode,
    ) -> CommandNode {
        if self.aliases.is_empty() {
            return variable_expanded;
        }

        variable_expanded.words = self.expand_aliases(&variable_expanded.words);
        variable_expanded
    }

    pub(in crate::executor) fn execute_function_command_invocation(
        &mut self,
        cmd: &CommandNode,
    ) -> Option<Result<(), ExecuteError>> {
        let function_name = cmd
            .words
            .first()
            .and_then(|word| self.function_name_for_command_word(word))?;
        let (materialized_cmd, process_substitution_files) =
            match self.command_with_process_substitution_files(cmd) {
                Ok(materialized) => materialized,
                Err(error) => return Some(Err(error)),
            };
        let temporary_assignments = self.apply_temporary_assignments(&materialized_cmd.assignments);
        let applied_assignment_values =
            self.applied_temporary_assignment_values(&materialized_cmd.assignments);
        let old_posix_export_touched = self.env_vars.remove(POSIX_FUNCTION_EXPORT_TOUCHED);
        let result = self.execute_function(
            &function_name,
            &materialized_cmd.words[1..],
            &materialized_cmd,
        );
        let finish_result = self.finish_process_substitutions(process_substitution_files);
        if self.posix_mode_enabled() {
            self.restore_function_temporary_assignments(
                temporary_assignments,
                applied_assignment_values,
            );
        } else {
            self.restore_temporary_assignments(temporary_assignments);
        }
        restore_optional_env_var(
            &mut self.env_vars,
            POSIX_FUNCTION_EXPORT_TOUCHED,
            old_posix_export_touched,
        );
        Some(result.and(finish_result))
    }

    pub(in crate::executor) fn execute_assignment_or_comment_command(
        &mut self,
        cmd: &CommandNode,
    ) -> bool {
        if self.execute_integer_assignment_suffix(cmd) || self.execute_assignment_words(cmd) {
            return true;
        }
        if self.execute_array_element_assignment(cmd) {
            return true;
        }
        if cmd.words.first().is_some_and(|word| word.starts_with('#')) {
            self.exit_code = 0;
            return true;
        }
        false
    }

    pub(in crate::executor) fn report_failglob(&mut self, pattern: &str) {
        eprintln!("{}no match: {pattern}", self.diagnostic_prefix());
        self.exit_code = 1;
    }
}

fn raw_word_contains_process_substitution(raw: Option<&str>) -> bool {
    raw.is_some_and(|raw| raw.contains("<(") || raw.contains(">("))
}

fn expanded_word_has_process_substitution(word: &str) -> bool {
    !WordMetadata::new(0, word.to_string(), word.to_string())
        .process_substitutions
        .is_empty()
}

pub(in crate::executor) fn expand_braces_with_optional_raw(
    word: &str,
    raw: Option<&str>,
) -> Vec<String> {
    if let Some(raw) = raw {
        if raw != word && !raw.contains("${") {
            let braced = crate::expand::braces::expand_braces(raw);
            if braced.len() > 1 {
                return braced
                    .into_iter()
                    .map(|word| crate::lexer::remove_shell_quotes(&word))
                    .collect();
            }
        }
    }

    crate::expand::braces::expand_braces(word)
}

fn raw_word_is_quoted(raw: Option<&str>) -> bool {
    let Some(raw) = raw else {
        return false;
    };
    let chars = raw.chars().collect::<Vec<_>>();
    let mut index = 0usize;
    while index < chars.len() {
        match chars[index] {
            '\'' | '"' => return true,
            '$' if chars.get(index + 1) == Some(&'\'') || chars.get(index + 1) == Some(&'"') => {
                return true;
            }
            '$' if chars.get(index + 1) == Some(&'(') => {
                index = skip_raw_command_substitution(&chars, index + 2);
                continue;
            }
            '`' => {
                index = skip_raw_backtick(&chars, index + 1);
                continue;
            }
            '\\' => index += 1,
            _ => {}
        }
        index += 1;
    }
    false
}

fn raw_has_quoted_empty_suffix(raw: &str) -> bool {
    raw.ends_with("''") || raw.ends_with("\"\"")
}

fn expanded_ends_with_ifs_separator(expanded: &str, executor: &Executor) -> bool {
    let Some(last) = expanded.chars().last() else {
        return false;
    };
    executor
        .env_vars
        .get("IFS")
        .map(String::as_str)
        .unwrap_or(" \t\n")
        .contains(last)
}

fn skip_raw_command_substitution(chars: &[char], mut index: usize) -> usize {
    let mut depth = 1usize;
    while index < chars.len() {
        match chars[index] {
            '\'' => index = skip_raw_quote(chars, index + 1, '\''),
            '"' => index = skip_raw_quote(chars, index + 1, '"'),
            '`' => index = skip_raw_backtick(chars, index + 1),
            '$' if chars.get(index + 1) == Some(&'(') => {
                depth += 1;
                index += 2;
                continue;
            }
            ')' => {
                depth = depth.saturating_sub(1);
                if depth == 0 {
                    return index + 1;
                }
            }
            '\\' => index += 1,
            _ => {}
        }
        index += 1;
    }
    index
}

fn skip_raw_quote(chars: &[char], mut index: usize, quote: char) -> usize {
    while index < chars.len() {
        if chars[index] == '\\' && quote != '\'' {
            index += 2;
            continue;
        }
        if chars[index] == quote {
            return index + 1;
        }
        index += 1;
    }
    index
}

fn skip_raw_backtick(chars: &[char], mut index: usize) -> usize {
    while index < chars.len() {
        if chars[index] == '\\' {
            index += 2;
            continue;
        }
        if chars[index] == '`' {
            return index + 1;
        }
        index += 1;
    }
    index
}
