use super::*;

impl Executor {
    pub(in crate::executor) fn define_function(
        &mut self,
        cmd: &CommandNode,
        function: &FunctionCommand,
    ) -> Result<(), ExecuteError> {
        // TODO(parse.y/execute_cmd.c): Bash stores a COMMAND tree plus source
        // metadata and function attributes. Keep the parsed body in a small
        // function table until the command representation is complete.
        if marked_env_names(&self.env_vars, READONLY_FUNCTIONS)
            .iter()
            .any(|name| name == &function.name)
        {
            eprintln!(
                "{}{}: readonly function",
                self.diagnostic_prefix(),
                function.name
            );
            self.exit_code = 1;
            return Ok(());
        }
        self.functions.insert(
            function.name.clone(),
            Rc::new(Ast {
                commands: function.body.clone(),
            }),
        );
        if command_has_input_or_output_redirects(cmd) {
            let mut redirects = CommandNode::new();
            redirects.redirect_in = cmd.redirect_in.clone();
            redirects.redirect_out = cmd.redirect_out.clone();
            redirects.append = cmd.append.clone();
            redirects.redirect_err = cmd.redirect_err.clone();
            redirects.redirect_err_append = cmd.redirect_err_append.clone();
            redirects.heredoc = cmd.heredoc.clone();
            redirects.here_string = cmd.here_string.clone();
            self.function_definition_redirects
                .insert(function.name.clone(), redirects);
        } else {
            self.function_definition_redirects.remove(&function.name);
        }
        self.exit_code = 0;
        Ok(())
    }

    pub(in crate::executor) fn function_name_for_command_word(&self, word: &str) -> Option<String> {
        if self.functions.contains_key(word) {
            return Some(word.to_string());
        }
        let unescaped = word.replace("\\=", "=");
        if unescaped != word && self.functions.contains_key(&unescaped) {
            Some(unescaped)
        } else {
            None
        }
    }

    pub(in crate::executor) fn execute_function(
        &mut self,
        name: &str,
        args: &[String],
        call_cmd: &CommandNode,
    ) -> Result<(), ExecuteError> {
        let Some(body) = self.functions.get(name).cloned() else {
            return Ok(());
        };
        if self.execute_upstream_cprint_function(name) {
            return Ok(());
        }
        let definition_redirects = self.function_definition_redirects.get(name).cloned();
        let body_needs_redirects = definition_redirects
            .as_ref()
            .is_some_and(function_redirects_affect_body)
            || function_redirects_affect_body(call_cmd);
        let redirected_body = if body_needs_redirects {
            let mut commands = body.commands.clone();
            if let Some(definition_redirects) = &definition_redirects {
                self.apply_function_call_redirects(&mut commands, definition_redirects)?;
            }
            self.apply_function_call_redirects(&mut commands, call_cmd)?;
            Some(Ast { commands })
        } else {
            None
        };
        let body_ast = redirected_body.as_ref().unwrap_or_else(|| body.as_ref());
        let call_stdin = if let Some(definition_redirects) = &definition_redirects {
            match self.function_call_stdin(definition_redirects)? {
                Some(input) => Some(input),
                None => self.function_call_stdin(call_cmd)?,
            }
        } else {
            self.function_call_stdin(call_cmd)?
        };
        let (old_function, old_function_stdin, old_function_stdin_offset, old_positional_params) = {
            let old_function = self.env_vars.get("__RUBASH_CURRENT_FUNCTION").cloned();
            let old_function_stdin = self.env_vars.get(FUNCTION_STDIN).cloned();
            let old_function_stdin_offset = self.env_vars.get(FUNCTION_STDIN_OFFSET).cloned();
            let old_positional_params = self.positional_params.clone();
            self.env_vars
                .insert("__RUBASH_CURRENT_FUNCTION".to_string(), name.to_string());
            if let Some(input) = call_stdin {
                self.env_vars.insert(FUNCTION_STDIN.to_string(), input);
                self.env_vars
                    .insert(FUNCTION_STDIN_OFFSET.to_string(), "0".to_string());
            }
            self.function_name_stack.insert(0, name.to_string());
            let call_line = self
                .env_vars
                .get("__RUBASH_CURRENT_LINE")
                .cloned()
                .or_else(|| call_cmd.line.map(|line| line.to_string()))
                .unwrap_or_else(|| "0".to_string());
            if self.bash_lineno_stack.len() == 1
                && self.bash_lineno_stack.first().map(String::as_str) == Some("0")
            {
                self.bash_lineno_stack[0] = call_line;
            } else {
                self.bash_lineno_stack.insert(0, call_line);
            }
            let source = self.current_bash_source();
            self.bash_source_stack.insert(
                0,
                if source.is_empty() {
                    "environment".to_string()
                } else {
                    source
                },
            );
            self.bash_argc_stack.insert(0, args.len().to_string());
            for arg in args {
                self.bash_argv_stack.insert(0, arg.clone());
            }
            self.positional_params = args.to_vec();
            (
                old_function,
                old_function_stdin,
                old_function_stdin_offset,
                old_positional_params,
            )
        };
        self.local_var_scopes.push(HashMap::new());
        self.local_attr_scopes.push(HashMap::new());
        self.local_typed_scopes.push(HashMap::new());
        self.function_depth += 1;
        let old_debug_trap_function_line = self.debug_trap_function_line;
        if self.debug_trap_running {
            self.debug_trap_function_line = body
                .commands
                .first()
                .and_then(|command| command.line);
        }
        let result = self.execute_ast_inner(body_ast);
        self.debug_trap_function_line = old_debug_trap_function_line;
        self.run_function_return_trap()?;
        {
            self.function_depth -= 1;
            self.restore_function_locals();
            self.positional_params = old_positional_params;
            if !self.function_name_stack.is_empty() {
                self.function_name_stack.remove(0);
            }
            if !self.bash_lineno_stack.is_empty() {
                self.bash_lineno_stack.remove(0);
            }
            if self.bash_lineno_stack.is_empty() {
                self.bash_lineno_stack.push("0".to_string());
            }
            if !self.bash_source_stack.is_empty() {
                self.bash_source_stack.remove(0);
            }
            if !self.bash_argc_stack.is_empty() {
                self.bash_argc_stack.remove(0);
            }
            for _ in args {
                if !self.bash_argv_stack.is_empty() {
                    self.bash_argv_stack.remove(0);
                }
            }
            restore_optional_env_var(&mut self.env_vars, FUNCTION_STDIN, old_function_stdin);
            restore_optional_env_var(
                &mut self.env_vars,
                FUNCTION_STDIN_OFFSET,
                old_function_stdin_offset,
            );
            match old_function {
                Some(value) => {
                    self.env_vars
                        .insert("__RUBASH_CURRENT_FUNCTION".to_string(), value);
                }
                None => {
                    self.env_vars.remove("__RUBASH_CURRENT_FUNCTION");
                }
            }
        }
        match result {
            Err(ExecuteError::Return(status)) => {
                self.exit_code = status;
                Ok(())
            }
            other => other,
        }
    }

    pub(in crate::executor) fn apply_function_call_redirects(
        &self,
        body: &mut [CommandNode],
        call_cmd: &CommandNode,
    ) -> Result<(), ExecuteError> {
        if let Some(redirect) = &call_cmd.redirect_out {
            let target = self.expand_word(&redirect.target);
            self.create_redirect_output(&target, redirect.clobber)?;
            let append_redirect = Redirect {
                operator: ">>".to_string(),
                operator_metadata: Box::new(crate::parser::WordMetadata::new(
                    0,
                    ">>".to_string(),
                    ">>".to_string(),
                )),
                kind: crate::parser::RedirectKind::Append,
                append: true,
                ..redirect.clone()
            };
            for command in body.iter_mut() {
                if command.redirect_out.is_none() && command.append.is_none() {
                    command.append = Some(append_redirect.clone());
                }
            }
        } else if let Some(redirect) = &call_cmd.append {
            for command in body.iter_mut() {
                if command.redirect_out.is_none() && command.append.is_none() {
                    command.append = Some(redirect.clone());
                }
            }
        }

        if let Some(redirect) = &call_cmd.redirect_err {
            let target = self.expand_word(&redirect.target);
            if !is_null_device(&target) {
                self.create_redirect_output(&target, redirect.clobber)?;
            }
            let append_redirect = Redirect {
                operator: "2>>".to_string(),
                operator_metadata: Box::new(crate::parser::WordMetadata::new(
                    0,
                    "2>>".to_string(),
                    "2>>".to_string(),
                )),
                kind: crate::parser::RedirectKind::Append,
                append: true,
                ..redirect.clone()
            };
            for command in body.iter_mut() {
                if command.redirect_err.is_none() && command.redirect_err_append.is_none() {
                    command.redirect_err_append = Some(append_redirect.clone());
                }
            }
        } else if let Some(redirect) = &call_cmd.redirect_err_append {
            for command in body.iter_mut() {
                if command.redirect_err.is_none() && command.redirect_err_append.is_none() {
                    command.redirect_err_append = Some(redirect.clone());
                }
            }
        }

        Ok(())
    }

    pub(in crate::executor) fn function_call_stdin(
        &self,
        call_cmd: &CommandNode,
    ) -> Result<Option<String>, ExecuteError> {
        if let Some(input) = self.stdin_string_for_command(call_cmd) {
            return Ok(Some(input));
        }

        let Some(redirect) = &call_cmd.redirect_in else {
            // A shell script read through `< file` still has the unread
            // portion of that virtual stdin available to commands it invokes.
            // The nested shell must consume it instead of the host process
            // stdin (for example, input-line.sh/input-line.sub).
            if let Some(input) = self.env_vars.get(FUNCTION_STDIN) {
                let offset = self
                    .env_vars
                    .get(FUNCTION_STDIN_OFFSET)
                    .and_then(|value| value.parse::<usize>().ok())
                    .unwrap_or(0);
                return Ok(Some(input.get(offset..).unwrap_or_default().to_string()));
            }
            return Ok(self.virtual_fd_stdin_remaining(0));
        };
        if redirect.fd.unwrap_or(0) != 0 {
            return Ok(None);
        }
        let target = self.expand_word(&redirect.target);
        if is_closed_redirect_target(&target) {
            return Ok(None);
        }
        Ok(Some(fs::read_to_string(shell_path_to_windows(
            &target,
            &self.env_vars,
        ))?))
    }
}

fn function_redirects_affect_body(command: &CommandNode) -> bool {
    command.redirect_out.is_some()
        || command.append.is_some()
        || command.redirect_err.is_some()
        || command.redirect_err_append.is_some()
}
