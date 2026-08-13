use super::*;
use crate::executor::parameter_core::word_contains_current_shell_command_substitution;

impl Executor {
    pub(in crate::executor) fn expand_embedded_parameters_mut(&mut self, word: &str) -> String {
        self.apply_parameter_assignment_expansions_in_word(word);
        let saved_parameter_state = word_contains_current_shell_command_substitution(word)
            .then(|| (self.env_vars.clone(), self.pipestatus.clone()));
        let expanded =
            self.expand_embedded_parameters_ordered_mut(word, saved_parameter_state.as_ref());
        let expanded = if word.contains("$(") || word.contains('`') {
            unescape_remaining_shell_escapes(&expanded)
                .replace("\\\\'", "'")
                .replace("\\'", "'")
        } else {
            expanded
        };
        restore_protected_replacement_quotes(&expanded)
            .replace('\x1f', "$")
            .replace('\x1a', "`")
            .replace('\x14', "\\")
    }

    fn expand_embedded_parameters_ordered_mut(
        &mut self,
        word: &str,
        saved_parameter_state: Option<&(std::collections::HashMap<String, String>, Vec<i32>)>,
    ) -> String {
        let mut output = String::new();
        let mut chars = word.chars().peekable();

        while let Some(ch) = chars.next() {
            if ch == '\x1a' {
                output.push('`');
                continue;
            }

            if ch == '\x1f' {
                output.push('$');
                continue;
            }

            if ch == '\x17' {
                output.push('\'');
                continue;
            }

            if ch == '\x18' {
                output.push('"');
                continue;
            }

            if ch == '`' {
                let mut source = String::new();
                let mut escaped = false;
                let mut closed = false;
                for source_ch in chars.by_ref() {
                    if escaped {
                        source.push(source_ch);
                        escaped = false;
                        continue;
                    }
                    if source_ch == '\\' {
                        escaped = true;
                        continue;
                    }
                    if source_ch == '`' {
                        closed = true;
                        break;
                    }
                    source.push(source_ch);
                }
                if closed {
                    output.push_str(&protect_command_substitution_output(
                        &self.expand_command_substitution_mut(&source),
                    ));
                } else {
                    output.push('`');
                    output.push_str(&source);
                }
                continue;
            }

            if ch != '$' {
                output.push(ch);
                continue;
            }

            match chars.peek().copied() {
                Some('?') => {
                    chars.next();
                    output.push_str(&self.exit_code.to_string());
                }
                Some('$') => {
                    chars.next();
                    output.push_str(&self.shell_pid_value().to_string());
                }
                Some('!') => {
                    chars.next();
                    output.push_str(&self.last_background_pid_value());
                }
                Some('@') => {
                    chars.next();
                    output.push_str(&self.positional_params.join(" "));
                }
                Some('*') => {
                    chars.next();
                    // Bash joins `$*` with the first IFS character (not a space).
                    output.push_str(&self.positional_params_star_joined());
                }
                Some('#') => {
                    chars.next();
                    output.push_str(&self.positional_params.len().to_string());
                }
                Some('-') => {
                    chars.next();
                    output.push_str(&self.shell_option_flags());
                }
                Some('{') => {
                    chars.next();
                    if let Some(value) = self.expand_current_shell_braced_substitution(&mut chars) {
                        output.push_str(&value);
                    } else {
                        let name = collect_braced_parameter_name(&mut chars);
                        output.push_str(
                            &self.expand_with_parameter_env(saved_parameter_state, |executor| {
                                executor.expand_word_mut(&format!("${{{name}}}"))
                            }),
                        );
                    }
                }
                Some('(') => {
                    chars.next();
                    if chars.peek().copied() == Some('(') {
                        chars.next();
                        let (expression, matched) =
                            collect_dollar_paren_arithmetic_expansion(&mut chars);
                        if matched {
                            if let Some(value) = self.eval_arithmetic_expansion_value(&expression) {
                                output.push_str(&value.to_string());
                            } else if !self.arithmetic_expansion_error.replace(true) {
                                let message = crate::executor::arithmetic::arithmetic_error_message(
                                    &expression,
                                )
                                .unwrap_or_else(|| {
                                    format!(
                                        "{expression}: syntax error in expression (error token is \"{expression}\")"
                                    )
                                });
                                eprintln!("{}{}", self.diagnostic_prefix(), message);
                            }
                        } else {
                            output.push_str("$((");
                            output.push_str(&expression);
                        }
                        continue;
                    }

                    let source = collect_command_substitution_source(&mut chars);
                    output.push_str(&protect_command_substitution_output(
                        &self.expand_command_substitution_mut(&source),
                    ));
                }
                Some('[') => {
                    chars.next();
                    let (expression, matched) =
                        collect_dollar_bracket_arithmetic_expansion(&mut chars);
                    if matched {
                        if let Some(value) = self.eval_arithmetic_expansion_value(&expression) {
                            output.push_str(&value.to_string());
                        }
                    } else {
                        output.push_str("$[");
                        output.push_str(&expression);
                    }
                }
                Some(first) if first.is_ascii_digit() => {
                    chars.next();
                    let index = first.to_digit(10).unwrap_or(0) as usize;
                    if index == 0 {
                        output.push_str(&self.script_name_value());
                    } else {
                        output.push_str(
                            self.positional_params
                                .get(index - 1)
                                .map(String::as_str)
                                .unwrap_or(""),
                        );
                    }
                }
                Some(first) if is_shell_name_start(first) => {
                    let mut name = String::new();
                    while let Some(name_ch) = chars.peek().copied() {
                        if !is_shell_name_char(name_ch) {
                            break;
                        }
                        chars.next();
                        name.push(name_ch);
                    }
                    if let Some(value) =
                        self.expand_with_parameter_env(saved_parameter_state, |executor| {
                            executor.dynamic_parameter_value(&name).or_else(|| {
                                executor
                                    .shell_variable_value(&name)
                                    .or_else(|| std::env::var(&name).ok())
                            })
                        })
                    {
                        output.push_str(&shell_safe_value(&value));
                    }
                }
                Some(other) => {
                    chars.next();
                    output.push('$');
                    output.push(other);
                }
                None => output.push('$'),
            }
        }

        output
    }

    fn expand_with_parameter_env<T>(
        &mut self,
        saved_parameter_state: Option<&(std::collections::HashMap<String, String>, Vec<i32>)>,
        expand: impl FnOnce(&mut Self) -> T,
    ) -> T {
        let Some((saved_parameter_env, saved_parameter_pipestatus)) = saved_parameter_state else {
            return expand(self);
        };

        let current_env = std::mem::replace(&mut self.env_vars, saved_parameter_env.clone());
        let current_pipestatus =
            std::mem::replace(&mut self.pipestatus, saved_parameter_pipestatus.clone());
        let expanded = expand(self);
        self.env_vars = current_env;
        self.pipestatus = current_pipestatus;
        expanded
    }

    fn expand_current_shell_braced_substitution(
        &mut self,
        chars: &mut std::iter::Peekable<std::str::Chars<'_>>,
    ) -> Option<String> {
        let pipe_output = chars.peek().copied() == Some('|');
        if pipe_output {
            chars.next();
        } else if !chars.peek().is_some_and(|ch| ch.is_whitespace()) {
            return None;
        }

        let mut depth = 1usize;
        let mut source = String::new();
        let mut single = false;
        let mut double = false;
        let mut escaped = false;
        let mut closed = false;
        for source_ch in chars.by_ref() {
            if escaped {
                source.push(source_ch);
                escaped = false;
                continue;
            }
            if source_ch == '\\' && !single {
                source.push(source_ch);
                escaped = true;
                continue;
            }
            match source_ch {
                '\'' if !double => {
                    single = !single;
                    source.push(source_ch);
                }
                '"' if !single => {
                    double = !double;
                    source.push(source_ch);
                }
                '{' if !single && !double => {
                    depth += 1;
                    source.push(source_ch);
                }
                '}' if !single && !double => {
                    depth = depth.saturating_sub(1);
                    if depth == 0 {
                        closed = true;
                        break;
                    }
                    source.push(source_ch);
                }
                _ => source.push(source_ch),
            }
        }

        if closed {
            Some(protect_command_substitution_output(
                &self.expand_current_shell_command_substitution(&source, pipe_output),
            ))
        } else {
            let mut literal = if pipe_output {
                "${|".to_string()
            } else {
                "${".to_string()
            };
            literal.push_str(&source);
            Some(literal)
        }
    }

    fn expand_current_shell_command_substitution(
        &mut self,
        source: &str,
        pipe_output: bool,
    ) -> String {
        let tokens = crate::lexer::tokenize(source);
        let ast = crate::parser::parse(&tokens);
        let saved_exit_code = self.exit_code;

        let (status, output) = if pipe_output {
            let result = self.execute_ast(&ast);
            let status = command_substitution_status(result, self.exit_code);
            (status, String::new())
        } else {
            let saved_capture = self.stdout_capture.take();
            self.stdout_capture = Some(Vec::new());
            let result = self.execute_ast(&ast);
            let status = command_substitution_status(result, self.exit_code);
            let output = String::from_utf8_lossy(&self.stdout_capture.take().unwrap_or_default())
                .trim_end_matches('\n')
                .to_string();
            self.stdout_capture = saved_capture;
            (status, output)
        };

        self.exit_code = saved_exit_code;
        self.last_command_substitution_status.set(Some(status));

        if pipe_output {
            self.env_vars.get("REPLY").cloned().unwrap_or_default()
        } else {
            output
        }
    }

    pub(in crate::executor) fn expand_command_substitution_mut(&mut self, source: &str) -> String {
        let source = source.trim();
        let words = self.expand_aliases(&split_shell_words(source));
        if let Some(output) = self.run_function_command_substitution(&words) {
            return output;
        }
        if command_substitution_words_contain_here_string(&words) {
            let alias_source = words.join(" ");
            if let Some(output) = self.run_ast_command_substitution(&alias_source) {
                return output;
            }
        }
        if command_substitution_uses_specialized_path(self, source, &words) {
            return self.expand_command_substitution(source);
        }
        // A command list (`echo a; echo b`, `a && b`) must run as an AST, not
        // through the single-command specialized dispatch below: routing
        // `echo mn; echo op` to the echo shortcut treats `;` as an argument
        // and yields `mn; echo op` instead of `mn\nop` (comsub.tests
        // `ab$(echo mn; echo op)yz`). A quoted `a;b` argument is harmless to
        // route here too: the AST still prints it correctly.
        if words
            .iter()
            .any(|word| word.contains(';') || matches!(word.as_str(), "&&" | "||"))
        {
            if let Some(output) = self.run_ast_command_substitution(source) {
                return output;
            }
        }
        // Simple builtins that the non-mut special-case dispatch handles with
        // proper quote stripping (echo/printf/cat/...). Prefer that path so
        // nested `"$(...)"` arguments do not leak quote characters through
        // the full-AST execution path.
        if is_specialized_command_substitution_word(&words) {
            return self.expand_command_substitution(source);
        }
        if let Some(output) = self.run_ast_command_substitution(source) {
            return output;
        }
        self.expand_command_substitution(source)
    }

    pub(in crate::executor) fn run_ast_command_substitution(
        &mut self,
        source: &str,
    ) -> Option<String> {
        if command_substitution_contains_heredoc(source) {
            return None;
        }

        let tokens = crate::lexer::tokenize(source);
        let ast = crate::parser::parse(&tokens);
        if !command_substitution_needs_ast_execution(&ast) {
            return None;
        }

        let saved_env = self.env_vars.clone();
        let saved_pipestatus = self.pipestatus.clone();
        let saved_functions = self.functions.clone();
        let saved_function_redirects = self.function_definition_redirects.clone();
        let saved_aliases = self.aliases.clone();
        let saved_exit_code = self.exit_code;
        let saved_dir = env::current_dir().ok();
        let saved_depth = self.subshell_depth.get();
        self.subshell_depth.set(saved_depth + 1);

        let saved_capture = self.stdout_capture.take();
        self.stdout_capture = Some(Vec::new());
        // Bash runs command substitution in a subshell where errexit is
        // suppressed: `$(false; echo ok)` prints ok because the inner `false`
        // does not abort the substitution (set-e.tests "command subst should
        // not inherit -e"); only the substitution's final status (echo's 0)
        // propagates to the outer assignment, which then checks -e.
        // POSIX mode is the exception: `set -o posix; z=$(false;echo posix)`
        // exits (set-e1.sub), so keep errexit active there.
        let posix_mode = self.env_vars.get("__RUBASH_POSIX_MODE").map(String::as_str) == Some("1");
        let result = if posix_mode {
            self.execute_ast(&ast)
        } else {
            self.with_errexit_suppressed(|executor| executor.execute_ast(&ast))
        };
        let output = self.stdout_capture.take().unwrap_or_default();
        self.stdout_capture = saved_capture;

        let status = match result {
            Ok(()) => self.exit_code,
            Err(ExecuteError::Return(status)) => status,
            Err(ExecuteError::ExitCode(status)) => status,
            Err(_) => 1,
        };

        self.restore_shell_env(saved_env);
        self.pipestatus = saved_pipestatus;
        self.functions = saved_functions;
        self.function_definition_redirects = saved_function_redirects;
        self.aliases = saved_aliases;
        if let Some(saved_dir) = saved_dir {
            let _ = env::set_current_dir(saved_dir);
        }
        self.subshell_depth.set(saved_depth);
        self.exit_code = saved_exit_code;
        self.last_command_substitution_status.set(Some(status));

        Some(
            String::from_utf8_lossy(&output)
                .trim_end_matches('\n')
                .to_string(),
        )
    }

    pub(in crate::executor) fn run_function_command_substitution(
        &mut self,
        words: &[String],
    ) -> Option<String> {
        let name = words.first()?;
        if !self.functions.contains_key(name) {
            return None;
        }

        let args = words[1..]
            .iter()
            .flat_map(|word| self.expand_command_substitution_arg_values(word))
            .collect::<Vec<_>>();
        let mut call = CommandNode::new();
        call.words = words.to_vec();

        let saved_env = self.env_vars.clone();
        let saved_pipestatus = self.pipestatus.clone();
        let saved_exit_code = self.exit_code;
        let saved_capture = self.stdout_capture.take();
        self.stdout_capture = Some(Vec::new());
        let result = self.execute_function(name, &args, &call);
        let output = self.stdout_capture.take().unwrap_or_default();
        self.stdout_capture = saved_capture;
        let status = match result {
            Ok(()) => self.exit_code,
            Err(ExecuteError::Return(status)) => status,
            Err(ExecuteError::ExitCode(status)) => status,
            Err(_) => 1,
        };
        self.env_vars = saved_env;
        self.pipestatus = saved_pipestatus;
        self.exit_code = saved_exit_code;
        self.last_command_substitution_status.set(Some(status));

        Some(
            String::from_utf8_lossy(&output)
                .trim_end_matches('\n')
                .to_string(),
        )
    }
}

fn command_substitution_needs_ast_execution(ast: &Ast) -> bool {
    ast.commands.iter().any(command_has_ast_substitution_shape)
        || ast
            .commands
            .iter()
            .any(command_contains_current_shell_substitution)
        || (ast.commands.len() > 1 && ast.commands.iter().all(command_is_ast_list_substitution))
}

fn collect_dollar_paren_arithmetic_expansion(
    chars: &mut std::iter::Peekable<std::str::Chars<'_>>,
) -> (String, bool) {
    let mut expression = String::new();
    let mut paren_depth: usize = 0;

    while let Some(ch) = chars.next() {
        match ch {
            '(' => {
                paren_depth += 1;
                expression.push(ch);
            }
            ')' if paren_depth == 0 && chars.peek().copied() == Some(')') => {
                chars.next();
                return (expression, true);
            }
            ')' => {
                paren_depth = paren_depth.saturating_sub(1);
                expression.push(ch);
            }
            _ => expression.push(ch),
        }
    }

    (expression, false)
}

fn collect_dollar_bracket_arithmetic_expansion(
    chars: &mut std::iter::Peekable<std::str::Chars<'_>>,
) -> (String, bool) {
    let mut expression = String::new();
    let mut bracket_depth: usize = 0;

    for ch in chars.by_ref() {
        match ch {
            '[' => {
                bracket_depth += 1;
                expression.push(ch);
            }
            ']' if bracket_depth == 0 => return (expression, true),
            ']' => {
                bracket_depth = bracket_depth.saturating_sub(1);
                expression.push(ch);
            }
            _ => expression.push(ch),
        }
    }

    (expression, false)
}

fn collect_command_substitution_source(
    chars: &mut std::iter::Peekable<std::str::Chars<'_>>,
) -> String {
    let mut depth = 1usize;
    let mut source = String::new();
    let mut single = false;
    let mut double = false;
    let mut escaped = false;
    let mut case_depth = 0usize;
    let mut word = String::new();
    let mut word_boundary = true;
    let mut current_word_boundary = true;

    while let Some(source_ch) = chars.next() {
        if escaped {
            source.push(source_ch);
            escaped = false;
            continue;
        }
        if source_ch == '\\' && !single {
            source.push(source_ch);
            escaped = true;
            continue;
        }
        if source_ch == '#' && !single && !double && word_boundary {
            source.push(source_ch);
            while let Some(comment_ch) = chars.peek().copied() {
                if comment_ch == '\n' {
                    break;
                }
                source.push(comment_ch);
                chars.next();
            }
            word.clear();
            word_boundary = true;
            current_word_boundary = true;
            continue;
        }
        let rest = chars.clone().collect::<String>();
        update_command_substitution_case_depth(
            source_ch,
            single,
            double,
            &mut word,
            &mut case_depth,
            &mut word_boundary,
            &mut current_word_boundary,
            &rest,
        );
        match source_ch {
            '\'' if !double => {
                single = !single;
                source.push(source_ch);
            }
            '"' if !single => {
                double = !double;
                source.push(source_ch);
            }
            '(' if !single && !double && case_depth == 0 => {
                depth += 1;
                source.push(source_ch);
            }
            ')' if !single && !double && case_depth == 0 => {
                depth = depth.saturating_sub(1);
                if depth == 0 {
                    break;
                }
                source.push(source_ch);
            }
            _ => source.push(source_ch),
        }
    }

    unescape_storage_command_substitution_source(&source)
}

fn command_substitution_status(result: Result<(), ExecuteError>, exit_code: i32) -> i32 {
    match result {
        Ok(()) => exit_code,
        Err(ExecuteError::Return(status)) => status,
        Err(ExecuteError::ExitCode(status)) => status,
        Err(_) => 1,
    }
}

fn command_has_ast_substitution_shape(command: &CommandNode) -> bool {
    command.and_or_list.is_some()
        || command.inverted_command.is_some()
        || command.background_command.is_some()
        || command_has_here_string_substitution(command)
        || command_has_compound_substitution(command)
}

fn command_has_compound_substitution(command: &CommandNode) -> bool {
    command.pipeline_command.as_ref().is_some_and(|pipeline| {
        pipeline
            .stages
            .iter()
            .any(command_has_compound_substitution)
    }) || command
        .and_or_list
        .as_ref()
        .is_some_and(|list| list.commands.iter().any(command_has_compound_substitution))
        || command
            .inverted_command
            .as_ref()
            .is_some_and(|inverted| command_has_compound_substitution(&inverted.command))
        || command
            .time_command
            .as_ref()
            .is_some_and(|time| command_has_compound_substitution(&time.command))
        || command.for_command.is_some()
        || command.if_command.is_some()
        || command.loop_command.is_some()
        || command.select_command.is_some()
        || command.case_command.is_some()
        || command.coproc_command.is_some()
        || command.subshell_command.is_some()
        || command.brace_group.is_some()
        || command.arithmetic_command.is_some()
        || command.conditional_command.is_some()
}

fn command_contains_current_shell_substitution(command: &CommandNode) -> bool {
    command
        .words
        .iter()
        .any(|word| word_contains_current_shell_command_substitution(word))
}

fn command_has_here_string_substitution(command: &CommandNode) -> bool {
    command.here_string.is_some()
        || command
            .heredoc_redirects
            .iter()
            .any(|redirect| redirect.here_string)
        || command.pipeline_command.as_ref().is_some_and(|pipeline| {
            pipeline
                .stages
                .iter()
                .any(command_has_here_string_substitution)
        })
        || command.and_or_list.as_ref().is_some_and(|list| {
            list.commands
                .iter()
                .any(command_has_here_string_substitution)
        })
        || command
            .inverted_command
            .as_ref()
            .is_some_and(|inverted| command_has_here_string_substitution(&inverted.command))
        || command
            .time_command
            .as_ref()
            .is_some_and(|time| command_has_here_string_substitution(&time.command))
}

fn command_is_ast_list_substitution(command: &CommandNode) -> bool {
    if !command_has_simple_substitution_shape(command) {
        return false;
    }
    if !command.assignments.is_empty() {
        return true;
    }
    matches!(
        command.words.first().map(String::as_str),
        Some("echo" | "printf" | "true" | "false" | ":" | "pwd")
    )
}

fn command_has_simple_substitution_shape(command: &CommandNode) -> bool {
    command.pipeline_command.is_none()
        && command.and_or_list.is_none()
        && command.inverted_command.is_none()
        && command.background_command.is_none()
        && command.time_command.is_none()
        && command.for_command.is_none()
        && command.if_command.is_none()
        && command.loop_command.is_none()
        && command.select_command.is_none()
        && command.case_command.is_none()
        && command.coproc_command.is_none()
        && command.subshell_command.is_none()
        && command.brace_group.is_none()
        && command.arithmetic_command.is_none()
        && command.conditional_command.is_none()
}

fn command_substitution_uses_specialized_path(
    executor: &Executor,
    source: &str,
    words: &[String],
) -> bool {
    command_substitution_contains_heredoc(source)
        || (words.iter().any(|word| word == "|")
            && !command_substitution_contains_here_string(source))
        || words.first().map(String::as_str) == Some("time")
        || executor
            .command_substitution_cd_pwd_output(source)
            .is_some()
}

fn command_substitution_words_contain_here_string(words: &[String]) -> bool {
    words
        .iter()
        .any(|word| word == "<<<" || word.ends_with("<<<"))
}

/// Builtin commands whose command-substitution output is produced by the
/// non-mut special-case dispatch in `expand_command_substitution` (echo,
/// printf, cat, basename, ...). Routing these through that path keeps nested
/// `"$(...)"` argument quote handling consistent with Bash.
fn is_specialized_command_substitution_word(words: &[String]) -> bool {
    matches!(
        words.first().map(String::as_str),
        Some(
            "echo"
                | "recho"
                | "printf"
                | "cat"
                | "basename"
                | "umask"
                | "ulimit"
                | "pwd"
                | "type"
                | "kill"
                | "trap"
                | "mktemp"
                | "set"
                | "export"
                | "true"
                | ":"
        )
    )
}

fn command_substitution_contains_here_string(source: &str) -> bool {
    let mut chars = source.chars().peekable();
    let mut single = false;
    let mut double = false;
    let mut escaped = false;

    while let Some(ch) = chars.next() {
        if escaped {
            escaped = false;
            continue;
        }
        if ch == '\\' && !single {
            escaped = true;
            continue;
        }
        match ch {
            '\'' if !double => single = !single,
            '"' if !single => double = !double,
            '<' if !single && !double && chars.peek().copied() == Some('<') => {
                chars.next();
                if chars.peek().copied() == Some('<') {
                    return true;
                }
            }
            _ => {}
        }
    }

    false
}

fn command_substitution_contains_heredoc(source: &str) -> bool {
    let mut chars = source.chars().peekable();
    let mut single = false;
    let mut double = false;
    let mut escaped = false;

    while let Some(ch) = chars.next() {
        if escaped {
            escaped = false;
            continue;
        }
        if ch == '\\' && !single {
            escaped = true;
            continue;
        }
        match ch {
            '\'' if !double => single = !single,
            '"' if !single => double = !double,
            '<' if !single && !double && chars.peek().copied() == Some('<') => {
                chars.next();
                if chars.peek().copied() == Some('<') {
                    chars.next();
                    continue;
                }
                return true;
            }
            _ => {}
        }
    }

    false
}

fn update_command_substitution_case_depth(
    ch: char,
    single: bool,
    double: bool,
    word: &mut String,
    case_depth: &mut usize,
    word_boundary: &mut bool,
    current_word_boundary: &mut bool,
    rest: &str,
) {
    if single || double {
        word.clear();
        *word_boundary = false;
        return;
    }

    if ch == '_' || ch.is_ascii_alphanumeric() {
        if word.is_empty() {
            *current_word_boundary = *word_boundary;
        }
        word.push(ch);
        return;
    }

    if word.is_empty() {
        if command_substitution_separator_allows_reserved_word(ch) {
            *word_boundary = true;
        } else if !ch.is_whitespace() {
            *word_boundary = false;
        }
        return;
    }

    let reserved_word_allows_next = match word.as_str() {
        "case" if *current_word_boundary => {
            *case_depth += 1;
            false
        }
        "esac" if *current_word_boundary && !case_pattern_starts_with_esac_rest(ch, rest) => {
            *case_depth = case_depth.saturating_sub(1);
            true
        }
        "for" | "select" | "while" | "until" | "then" | "do" | "else" | "elif" | "in" | "fi"
        | "done"
            if *current_word_boundary =>
        {
            true
        }
        _ => false,
    };
    word.clear();
    *word_boundary =
        reserved_word_allows_next || command_substitution_separator_allows_reserved_word(ch);
}

fn command_substitution_separator_allows_reserved_word(ch: char) -> bool {
    matches!(ch, ';' | '&' | '|' | '(' | ')' | '\n')
}

fn case_pattern_starts_with_esac_rest(delimiter: char, rest: &str) -> bool {
    if !matches!(delimiter, ')' | '|') {
        return false;
    }

    let chars = std::iter::once(delimiter)
        .chain(rest.chars())
        .collect::<Vec<_>>();
    let mut close = 0usize;
    while close < chars.len() {
        match chars[close] {
            ')' => break,
            ';' | '\n' => return false,
            _ => close += 1,
        }
    }
    if chars.get(close) != Some(&')') {
        return false;
    }

    let mut scan = close + 1;
    let mut word = String::new();
    let mut word_boundary = true;
    while scan < chars.len() {
        let ch = chars[scan];
        if ch == ';' && chars.get(scan + 1) == Some(&';') {
            return true;
        }
        if ch == '_' || ch.is_ascii_alphanumeric() {
            word.push(ch);
            scan += 1;
            continue;
        }
        if word == "esac" && word_boundary {
            return true;
        }
        if ch == ')' {
            return false;
        }
        if word.is_empty() {
            if command_substitution_separator_allows_reserved_word(ch) {
                word_boundary = true;
            } else if !ch.is_whitespace() {
                word_boundary = false;
            }
            scan += 1;
            continue;
        }
        let reserved_word_allows_next =
            word_boundary && command_substitution_reserved_word_allows_next(&word);
        word.clear();
        word_boundary =
            reserved_word_allows_next || command_substitution_separator_allows_reserved_word(ch);
        scan += 1;
    }

    word == "esac" && word_boundary
}

fn command_substitution_reserved_word_allows_next(word: &str) -> bool {
    matches!(
        word,
        "for"
            | "select"
            | "while"
            | "until"
            | "then"
            | "do"
            | "else"
            | "elif"
            | "in"
            | "fi"
            | "done"
            | "esac"
    )
}
