use super::*;

impl Executor {
    pub(in crate::executor) fn execute_array_element_assignment(
        &mut self,
        cmd: &CommandNode,
    ) -> bool {
        // TODO(variables.c/array.c/assoc.c): Bash array element assignment
        // carries typed SHELL_VAR attributes. This stores the element count
        // shape needed by upstream builtins5.sub.
        if cmd.words.len() != 1 {
            if !cmd
                .words
                .iter()
                .all(|word| is_array_element_assignment_word(word))
            {
                return false;
            }
            for word in &cmd.words {
                let mut single = cmd.clone();
                single.words = vec![word.clone()];
                single.array_element_assignments = cmd
                    .array_element_assignments
                    .iter()
                    .filter(|assignment| {
                        assignment
                            .word_index
                            .and_then(|word_index| cmd.words.get(word_index))
                            .is_some_and(|assignment_word| assignment_word == word)
                    })
                    .cloned()
                    .map(|mut assignment| {
                        assignment.word_index = Some(0);
                        assignment
                    })
                    .collect();
                if !self.execute_array_element_assignment(&single) {
                    return false;
                }
                if self.exit_code != 0 {
                    return true;
                }
            }
            self.exit_code = 0;
            return true;
        }
        if !is_array_element_assignment_word(&cmd.words[0]) {
            return false;
        }
        let Some((left, value)) = cmd.words[0].split_once('=') else {
            return false;
        };
        let (left, append) = if let Some(left) = left.strip_suffix('+') {
            (left, true)
        } else {
            (left, false)
        };
        let Some((name, index)) = left.split_once('[') else {
            return false;
        };
        if !index.ends_with(']') || !is_shell_name(name) {
            return false;
        }
        let element_assignment = cmd.array_element_assignments.iter().find(|assignment| {
            assignment
                .word_index
                .and_then(|word_index| cmd.words.get(word_index))
                .is_some_and(|word| word == &cmd.words[0])
                && assignment.name == name
        });
        let raw_subscript =
            element_assignment.map(|assignment| assignment.subscript_metadata.raw.as_str());
        let value_is_syntactic_compound_list = element_assignment.is_some_and(|assignment| {
            let raw_value = assignment.value.trim();
            raw_value.starts_with('(')
                && raw_value.ends_with(')')
                && assignment.word_quotes.is_empty()
        });
        let name = match self.nameref_resolution(name) {
            NamerefResolution::Target(target) => target,
            NamerefResolution::Circular => {
                eprintln!(
                    "{}warning: {}: circular name reference",
                    self.diagnostic_prefix(),
                    name
                );
                self.exit_code = 1;
                return true;
            }
            NamerefResolution::NotNameref => name.to_string(),
        };
        let name = name.as_str();
        if name == "BASH_ALIASES" {
            // TODO(variables.c/alias.c): BASH_ALIASES is a dynamic
            // associative array backed by the alias table. Keep this narrow
            // bridge here so array assignment does not swallow alias.tests'
            // invalid-name diagnostic.
            let alias_name = index
                .trim_end_matches(']')
                .trim_matches('\'')
                .trim_matches('"');
            if !valid_alias_assignment_name(alias_name) {
                eprintln!(
                    "{}`{alias_name}': invalid alias name",
                    self.diagnostic_prefix()
                );
                self.exit_code = 1;
                return true;
            }
            self.aliases
                .insert(alias_name.to_string(), Alias::new(value));
            self.sync_dynamic_assoc_vars();
            self.exit_code = 0;
            return true;
        }
        if name == "DIRSTACK" {
            // TODO(builtins/pushd.def/variables.c): Bash exposes the
            // directory stack as a dynamic array variable. Keep assignments
            // wired to the pushd module's stack storage until SHELL_VAR array
            // attributes are ported.
            let Some(index) = index.trim_end_matches(']').parse::<usize>().ok() else {
                self.exit_code = 1;
                return true;
            };
            crate::builtins::pushd::set_stack_value(&mut self.env_vars, index, value.to_string());
            self.exit_code = 0;
            return true;
        }
        if name == "GROUPS" {
            self.exit_code = 0;
            return true;
        }
        if is_noassign_bash_array(name) {
            self.exit_code = 0;
            return true;
        }
        if name == "BASH_CMDS" {
            let command_name = index
                .trim_end_matches(']')
                .trim_matches('\'')
                .trim_matches('"');
            crate::builtins::hash::set_hashed_path(&mut self.env_vars, command_name, value);
            self.sync_dynamic_assoc_vars();
            self.exit_code = 0;
            return true;
        }

        let index = index.trim_end_matches(']');
        if value_is_syntactic_compound_list {
            eprintln!(
                "{}{}: cannot assign list to array member",
                self.diagnostic_prefix(),
                cmd.words[0]
            );
            self.exit_code = 1;
            return true;
        }
        if is_marked_var(&self.env_vars, READONLY_VARS, name) {
            eprintln!("{}{}: readonly variable", self.diagnostic_prefix(), name);
            self.exit_code = 1;
            return true;
        }
        if is_marked_var(&self.env_vars, ASSOC_VARS, name) || self.is_assoc_parameter_array(name) {
            // TODO(assoc.c/arrayfunc.c): Bash parses associative subscripts
            // with quote removal and expansion. This stores the simple
            // `A[key]=value` form exercised by upstream builtins5.sub.
            let key = self.assoc_subscript_key(raw_subscript.unwrap_or(index));
            let current = self.env_vars.get(name).cloned().unwrap_or_default();
            let mut entries = assoc_entries(&current);
            let value = if append {
                let current = entries
                    .iter()
                    .rev()
                    .find_map(|(entry_key, entry_value)| {
                        (entry_key == &key).then_some(entry_value.as_str())
                    })
                    .unwrap_or_default();
                append_scalar_value(current, value)
            } else {
                value.to_string()
            };
            if let Some((_, entry_value)) = entries
                .iter_mut()
                .rev()
                .find(|(entry_key, _)| entry_key == &key)
            {
                *entry_value = value;
            } else {
                entries.push((key, value));
            }
            let new_value = format!(
                "({})",
                entries
                    .into_iter()
                    .map(|(key, value)| {
                        format!(
                            "[{}]={}",
                            quote_assoc_key(&key),
                            quote_assoc_storage_value(&value)
                        )
                    })
                    .collect::<Vec<_>>()
                    .join(" ")
            );
            self.env_vars.insert(name.to_string(), new_value);
            self.exit_code = 0;
            return true;
        }

        if index.trim().is_empty()
            && raw_subscript
                .map(str::trim)
                .is_none_or(|raw| raw.is_empty() || raw == "[]")
        {
            eprintln!(
                "{}{}: bad array subscript",
                self.diagnostic_prefix(),
                cmd.words[0]
            );
            self.exit_code = 1;
            return true;
        }
        let index = if index.trim().is_empty() { "0" } else { index };
        if index.trim() == "*" {
            eprintln!(
                "{}{}: bad array subscript",
                self.diagnostic_prefix(),
                cmd.words[0]
            );
            self.exit_code = 1;
            return true;
        }
        let Some(computed_index) = self.eval_arithmetic_expansion_value(index) else {
            eprintln!(
                "{}{}: bad array subscript",
                self.diagnostic_prefix(),
                cmd.words[0]
            );
            self.exit_code = 1;
            return true;
        };
        if computed_index < 0
            && resolve_indexed_array_subscript(
                &self.env_vars.get(name).cloned().unwrap_or_default(),
                computed_index,
            )
            .is_none()
        {
            eprintln!(
                "{}{}: bad array subscript",
                self.diagnostic_prefix(),
                cmd.words[0]
            );
            self.exit_code = 1;
            return true;
        }

        let current = self.env_vars.get(name).cloned().unwrap_or_default();
        let index = if computed_index < 0 {
            let Some(index) = resolve_indexed_array_subscript(&current, computed_index) else {
                eprintln!(
                    "{}{}: bad array subscript",
                    self.diagnostic_prefix(),
                    cmd.words[0]
                );
                self.exit_code = 1;
                return true;
            };
            index
        } else {
            let Ok(index) = usize::try_from(computed_index) else {
                return false;
            };
            index
        };
        let mut entries = indexed_array_entries(&current);
        let current_element = entries.get(&index).cloned().unwrap_or_default();
        let element = if append {
            if is_marked_var(&self.env_vars, INTEGER_VARS, name) {
                (eval_arith_value(&current_element) + eval_arith_value(value)).to_string()
            } else {
                append_scalar_value(&current_element, value)
            }
        } else {
            value.to_string()
        };
        let element = if is_marked_var(&self.env_vars, INTEGER_VARS, name) {
            eval_arith_value(&element).to_string()
        } else {
            element
        };
        entries.insert(index, element);
        self.env_vars
            .insert(name.to_string(), format_indexed_array_storage(entries));
        mark_env_name(&mut self.env_vars, ARRAY_VARS, name);
        self.exit_code = 0;
        true
    }
}
