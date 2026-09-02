use super::*;

impl Executor {
    pub(in crate::executor) fn execute_readonly(
        &mut self,
        cmd: &CommandNode,
    ) -> Result<i32, ExecuteError> {
        if readonly_args_request_functions(&cmd.words[1..]) {
            let mut stdout = Vec::new();
            let mut stderr = Vec::new();
            let status =
                self.execute_readonly_functions(&cmd.words[1..], &mut stdout, &mut stderr)?;
            self.write_buffered_builtin_output(cmd, &stdout, &stderr)?;
            return Ok(status);
        }

        let mut stdout = Vec::new();
        let mut stderr = Vec::new();
        let status = crate::builtins::setattr::readonly_with_io(
            cmd.words[1..].iter().map(String::as_str),
            &mut self.env_vars,
            &mut stdout,
            &mut stderr,
        )?;
        if status == 0 {
            self.sync_setattr_typed_assignments(cmd.words[1..].iter().map(String::as_str));
        }
        self.write_buffered_builtin_output(cmd, &stdout, &stderr)?;
        Ok(status)
    }

    pub(in crate::executor) fn execute_readonly_functions<W, E>(
        &mut self,
        args: &[String],
        stdout: &mut W,
        stderr: &mut E,
    ) -> io::Result<i32>
    where
        W: Write,
        E: Write,
    {
        let mut print = false;
        let mut index = 0;
        while let Some(arg) = args.get(index) {
            if arg == "--" {
                index += 1;
                break;
            }
            if !arg.starts_with('-') || arg == "-" {
                break;
            }
            for option in arg[1..].chars() {
                match option {
                    'f' => {}
                    'p' => print = true,
                    'a' | 'A' => {}
                    other => {
                        writeln!(
                            stderr,
                            "{}readonly: -{other}: invalid option",
                            self.diagnostic_prefix()
                        )?;
                        writeln!(
                            stderr,
                            "readonly: usage: readonly [-aAf] [name[=value] ...] or readonly -p"
                        )?;
                        return Ok(2);
                    }
                }
            }
            index += 1;
        }

        if print && index >= args.len() {
            let mut names = marked_env_names(&self.env_vars, READONLY_FUNCTIONS);
            names.sort();
            for name in names {
                if let Some(body) = self.functions.get(&name) {
                    self.write_function_definition(&name, &body.commands, false, stdout)?;
                    writeln!(stdout, "declare -fr {name}")?;
                }
            }
            return Ok(0);
        }

        let mut status = 0;
        for name in &args[index..] {
            let Some(body) = self.functions.get(name) else {
                writeln!(
                    stderr,
                    "{}readonly: {name}: not a function",
                    self.diagnostic_prefix()
                )?;
                status = 1;
                continue;
            };
            if print {
                self.write_function_definition(name, &body.commands, false, stdout)?;
                writeln!(stdout, "declare -fr {name}")?;
            }
            mark_env_name(&mut self.env_vars, READONLY_FUNCTIONS, name);
        }

        Ok(status)
    }

    pub(in crate::executor) fn write_function_definition<W>(
        &self,
        name: &str,
        body: &[CommandNode],
        exported: bool,
        stdout: &mut W,
    ) -> io::Result<()>
    where
        W: Write,
    {
        if exported {
            writeln!(stdout, "declare -fx {name}")?;
        }
        writeln!(stdout, "{name} () ")?;
        writeln!(stdout, "{{ ")?;
        let printable_commands = body
            .iter()
            .filter(|command| function_definition_command_is_printable(command))
            .collect::<Vec<_>>();
        let last_index = printable_commands.len().saturating_sub(1);
        let mut indent_level = 1usize;
        let mut previous_first_word: Option<String> = None;
        for (index, command) in printable_commands.iter().enumerate() {
            let first_word = command.words.first().map(String::as_str);
            let next_first_word = printable_commands
                .get(index + 1)
                .and_then(|next| next.words.first())
                .map(String::as_str);
            let indent = "    ".repeat(indent_level);
            if function_definition_condition_needs_if_keyword(
                previous_first_word.as_deref(),
                first_word,
                next_first_word,
            ) {
                writeln!(stdout, "{indent}if")?;
            } else if function_definition_condition_needs_while_keyword(first_word, next_first_word)
            {
                writeln!(stdout, "{indent}while")?;
            }

            if let Some(if_command) = &command.if_command {
                self.write_if_function_definition(if_command, stdout, indent_level)?;
                continue;
            }
            if let Some(loop_command) = &command.loop_command {
                self.write_loop_function_definition(loop_command, stdout, indent_level)?;
                continue;
            }
            if function_definition_command_closes_block(command) {
                indent_level = indent_level.saturating_sub(1).max(1);
            }
            let indent = "    ".repeat(indent_level);
            let terminator =
                if function_definition_command_omits_terminator(command) || index == last_index {
                    ""
                } else {
                    ";"
                };
            if let Some(here_string) = &command.here_string {
                writeln!(
                    stdout,
                    "{indent}{} <<< {}{}",
                    function_definition_source_line(command.words.join(" ")),
                    function_here_string_text(here_string, printable_commands.len() > 1),
                    terminator
                )?;
            } else if command.words == ["time"] {
                writeln!(stdout, "{indent}time {terminator}")?;
            } else if command.heredoc.is_some() {
                let line = self
                    .function_command_description_line(command, false)
                    .unwrap_or_else(|| command.words.join(" "));
                let line = function_definition_source_line(line);
                writeln!(stdout, "{indent}{line}")?;
                write_function_definition_heredoc_body(command, stdout)?;
            } else {
                let line = if function_definition_command_uses_source_text(command) {
                    bash_command_source_text(command)
                } else {
                    command.words.join(" ")
                };
                let line = function_definition_source_line(line);
                if line.trim().is_empty() {
                    continue;
                }
                writeln!(stdout, "{indent}{line}{terminator}")?;
            }
            if function_definition_command_opens_nested_body(command) {
                indent_level += 1;
            }
            previous_first_word = first_word.map(str::to_string);
        }
        writeln!(stdout, "}}")
    }

    fn write_if_function_definition<W>(
        &self,
        if_command: &IfCommand,
        stdout: &mut W,
        indent_level: usize,
    ) -> io::Result<()>
    where
        W: Write,
    {
        self.write_if_condition_definition("if", &if_command.condition, stdout, indent_level)?;
        self.write_function_definition_commands(&if_command.then_body, stdout, indent_level + 1)?;
        for branch in &if_command.elif_branches {
            self.write_if_condition_definition("elif", &branch.condition, stdout, indent_level)?;
            self.write_function_definition_commands(&branch.body, stdout, indent_level + 1)?;
        }
        if let Some(body) = &if_command.else_body {
            writeln!(stdout, "{}else", "    ".repeat(indent_level))?;
            self.write_function_definition_commands(body, stdout, indent_level + 1)?;
        }
        writeln!(stdout, "{}fi", "    ".repeat(indent_level))
    }

    fn write_if_condition_definition<W>(
        &self,
        keyword: &str,
        condition: &[CommandNode],
        stdout: &mut W,
        indent_level: usize,
    ) -> io::Result<()>
    where
        W: Write,
    {
        let indent = "    ".repeat(indent_level);
        let Some((first, rest)) = condition.split_first() else {
            writeln!(stdout, "{indent}{keyword}")?;
            writeln!(stdout, "{indent}then")?;
            return Ok(());
        };

        let line = self
            .function_command_description_line(first, false)
            .unwrap_or_else(|| first.words.join(" "));
        let line = function_definition_source_line(line);
        let line = function_definition_prefix_condition_keyword(keyword, line);
        let line = if first.heredoc.is_some() {
            line
        } else {
            function_definition_condition_line(line)
        };
        writeln!(stdout, "{indent}{line}")?;
        write_function_definition_heredoc_body(first, stdout)?;
        for command in rest {
            self.write_function_definition_command(command, stdout, indent_level)?;
        }
        writeln!(stdout, "{indent}then")
    }

    fn write_function_definition_commands<W>(
        &self,
        commands: &[CommandNode],
        stdout: &mut W,
        indent_level: usize,
    ) -> io::Result<()>
    where
        W: Write,
    {
        for command in commands
            .iter()
            .filter(|command| function_definition_command_is_printable(command))
        {
            if let Some(if_command) = &command.if_command {
                self.write_if_function_definition(if_command, stdout, indent_level)?;
            } else if let Some(loop_command) = &command.loop_command {
                self.write_loop_function_definition(loop_command, stdout, indent_level)?;
            } else {
                self.write_function_definition_command(command, stdout, indent_level)?;
            }
        }
        Ok(())
    }

    fn write_loop_function_definition<W>(
        &self,
        loop_command: &LoopCommand,
        stdout: &mut W,
        indent_level: usize,
    ) -> io::Result<()>
    where
        W: Write,
    {
        self.write_loop_condition_definition(loop_command, stdout, indent_level)?;
        self.write_function_definition_commands(&loop_command.body, stdout, indent_level + 1)?;
        writeln!(stdout, "{}done", "    ".repeat(indent_level))
    }

    fn write_loop_condition_definition<W>(
        &self,
        loop_command: &LoopCommand,
        stdout: &mut W,
        indent_level: usize,
    ) -> io::Result<()>
    where
        W: Write,
    {
        let keyword = if loop_command.until { "until" } else { "while" };
        let indent = "    ".repeat(indent_level);
        let Some((first, rest)) = loop_command.condition.split_first() else {
            writeln!(stdout, "{indent}{keyword}")?;
            writeln!(stdout, "{indent}do")?;
            return Ok(());
        };

        let line = self
            .function_command_description_line(first, false)
            .unwrap_or_else(|| first.words.join(" "));
        let line = function_definition_source_line(line);
        let line = function_definition_prefix_condition_keyword(keyword, line);
        let line = if first.heredoc.is_some() {
            line
        } else {
            function_definition_condition_line(line)
        };
        writeln!(stdout, "{indent}{line}")?;
        write_function_definition_heredoc_body(first, stdout)?;
        for command in rest {
            self.write_function_definition_command(command, stdout, indent_level)?;
        }
        writeln!(stdout, "{indent}do")
    }

    fn write_function_definition_command<W>(
        &self,
        command: &CommandNode,
        stdout: &mut W,
        indent_level: usize,
    ) -> io::Result<()>
    where
        W: Write,
    {
        let indent = "    ".repeat(indent_level);
        if command.heredoc.is_some() {
            let line = self
                .function_command_description_line(command, false)
                .unwrap_or_else(|| command.words.join(" "));
            let line = function_definition_source_line(line);
            writeln!(stdout, "{indent}{line}")?;
            write_function_definition_heredoc_body(command, stdout)
        } else {
            let line = if function_definition_command_uses_source_text(command) {
                bash_command_source_text(command)
            } else {
                command.words.join(" ")
            };
            let line = function_definition_source_line(line);
            if line.trim().is_empty() {
                return Ok(());
            }
            writeln!(stdout, "{indent}{line};")
        }
    }

    pub(in crate::executor) fn apply_exported_functions_to_child(&self, process: &mut Command) {
        for name in marked_env_names(&self.env_vars, EXPORTED_FUNCTIONS) {
            let Some(body) = self.functions.get(&name) else {
                continue;
            };
            process.env(
                exported_function_env_name(&name),
                exported_function_env_value(&body.commands),
            );
        }
    }

    pub(in crate::executor) fn apply_child_environment(&self, process: &mut Command) {
        process.env_clear();
        for name in marked_env_names(&self.env_vars, EXPORTED_VARS) {
            if let Some(value) = self.env_vars.get(&name) {
                if is_valid_process_env(&name, value) {
                    process.env(&name, self.child_env_value(&name, value));
                }
            }
        }
        for (name, value) in local_export_env_values(&self.env_vars) {
            if is_valid_process_env(&name, &value) {
                process.env(&name, self.child_env_value(&name, &value));
            }
        }
        apply_required_windows_child_environment(process, &self.env_vars);
        self.apply_exported_functions_to_child(process);
    }

    pub(in crate::executor) fn child_env_value(&self, name: &str, value: &str) -> String {
        if cfg!(windows) && name.eq_ignore_ascii_case("PATH") {
            return shell_path_to_process(value, &self.env_vars);
        }
        if cfg!(windows) && name == "TMPDIR" {
            return shell_display_path(
                &shell_path_to_windows(value, &self.env_vars)
                    .to_string_lossy()
                    .replace('\\', "/"),
            );
        }
        value.to_string()
    }
}

fn function_definition_source_line(line: String) -> String {
    eval_source_for_reparse(&line)
}

fn function_definition_prefix_condition_keyword(keyword: &str, line: String) -> String {
    if line.trim_start().starts_with(keyword) {
        line
    } else {
        format!("{keyword} {line}")
    }
}

fn function_definition_condition_line(line: String) -> String {
    if line.trim_end().ends_with(';') {
        line
    } else {
        format!("{line};")
    }
}

fn function_definition_condition_needs_if_keyword(
    previous_first_word: Option<&str>,
    first_word: Option<&str>,
    next_first_word: Option<&str>,
) -> bool {
    next_first_word == Some("then")
        && previous_first_word != Some("elif")
        && !matches!(first_word, Some("if" | "elif" | "then" | "else" | "fi"))
}

fn function_definition_condition_needs_while_keyword(
    first_word: Option<&str>,
    next_first_word: Option<&str>,
) -> bool {
    next_first_word == Some("do")
        && !matches!(
            first_word,
            Some("for" | "while" | "until" | "select" | "do" | "done")
        )
}
