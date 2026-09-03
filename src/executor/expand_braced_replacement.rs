use super::*;

impl Executor {
    pub(in crate::executor) fn expand_braced_replacement_parameter(
        &self,
        name: &str,
    ) -> Option<String> {
        let (var_name, pattern, replacement, global) = parse_parameter_replacement(name)?;
        // GNU subst.c match_upattern applies FNMATCH_IGNCASE when nocasematch
        // is set, so pattern substitution honors the shopt (bash 4.3+).
        let nocase =
            crate::builtins::shopt::option_enabled(&self.env_vars, "nocasematch");
        let pattern = self.expand_parameter_pattern_word(
            &pattern
                .replace(r"\/", "/")
                .replace('\x14', "/")
                .replace('\x18', "/"),
        );
        let replacement = decode_parameter_replacement_quotes(
            &self.expand_embedded_parameters_preserving_escaped_single_quotes(replacement),
        );
        if let Some(value) =
            self.indirect_replacement_parameter(var_name, &pattern, &replacement, global)
        {
            return Some(value);
        }
        if matches!(var_name, "@" | "*") {
            return Some(
                self.positional_params
                    .iter()
                    .map(|value| {
                        replace_parameter_pattern(value, &pattern, &replacement, global, nocase)
                    })
                    .collect::<Vec<_>>()
                    .join(" "),
            );
        }
        if let Ok(index) = var_name.parse::<usize>() {
            return Some(
                self.positional_params
                    .get(index.saturating_sub(1))
                    .map(|value| {
                        replace_parameter_pattern(
                            &value.replace('\x1b', ""),
                            &pattern,
                            &replacement,
                            global,
                            nocase,
                        )
                    })
                    .unwrap_or_default(),
            );
        }
        if let Some(value) = self.array_element_parameter_value(var_name) {
            return Some(replace_parameter_pattern(
                &value,
                &pattern,
                &replacement,
                global,
                nocase,
            ));
        }
        if let Some(array_name) = var_name
            .strip_suffix("[@]")
            .or_else(|| var_name.strip_suffix("[*]"))
        {
            return Some(
                self.env_vars
                    .get(array_name)
                    .map(|value| {
                        let values = array_values(value)
                            .into_iter()
                            .map(|value| {
                                replace_parameter_pattern(
                                    &value,
                                    &pattern,
                                    &replacement,
                                    global,
                                    nocase,
                                )
                            })
                            .collect::<Vec<_>>();
                        self.join_expanded_array_values(values, var_name)
                    })
                    .unwrap_or_default(),
            );
        }
        if is_shell_name(var_name) {
            // GNU variables.c get_string_value: a bare array name expands to
            // element [0], so pattern substitution must operate on that
            // element (${av/??/xx} -> "xxcd"), not the raw typed storage
            // (previously leaked `xx[0]="abcd"` fragments). This also keeps
            // the nameref chain resolution the old inline code performed.
            return Some(
                self.parameter_pattern_scalar_value(var_name)
                    .map(|value| {
                        replace_parameter_pattern(&value, &pattern, &replacement, global, nocase)
                    })
                    .unwrap_or_default(),
            );
        }
        None
    }

    fn indirect_replacement_parameter(
        &self,
        var_name: &str,
        pattern: &str,
        replacement: &str,
        global: bool,
    ) -> Option<String> {
        let indirect_name = var_name.strip_prefix('!')?;
        if let Some(target_name) = self.nameref_target_name(indirect_name) {
            return Some(replace_parameter_pattern(
                &target_name,
                pattern,
                replacement,
                global,
                self.nocasematch_enabled(),
            ));
        }

        let target_expr = self.env_vars.get(indirect_name)?;
        let values = self.indirect_target_values(target_expr);
        if values.is_empty() {
            return Some(String::new());
        }

        let values = values
            .into_iter()
            .map(|value| {
                replace_parameter_pattern(&value, pattern, replacement, global, self.nocasematch_enabled())
            })
            .collect::<Vec<_>>();
        Some(self.join_expanded_array_values(values, target_expr))
    }
}
