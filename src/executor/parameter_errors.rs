use super::*;
use crate::executor::parameter_core::word_contains_current_shell_command_substitution;

/// Recognize `=` / `:=` whose parameter name is a bare special parameter
/// (`!`, `@`, `*`). GNU subst.c parameter_brace_expand treats these like
/// positional parameters for the assignment operators (subst.c:10404 reports
/// "$%s: cannot assign in this way" for them); plain variable names and
/// numeric positionals are handled by parse_parameter_assignment_operator.
fn parse_special_assignment_operator(inner: &str) -> Option<(&str, bool)> {
    if let Some((name, _)) = inner.split_once(":=") {
        if matches!(name, "!" | "@" | "*") {
            return Some((name, true));
        }
    }
    if let Some((name, _)) = inner.split_once('=') {
        if matches!(name, "!" | "@" | "*") {
            return Some((name, false));
        }
    }
    None
}

impl Executor {
    pub(in crate::executor) fn parameter_assignment_error(
        &self,
        cmd: &CommandNode,
    ) -> Option<(String, &'static str)> {
        for word in &cmd.words {
            if let Some(error) = self.parameter_assignment_error_in_word(word) {
                return Some(error);
            }
        }
        for value in cmd.assignment_values() {
            if let Some(error) = self.parameter_assignment_error_in_word(value) {
                return Some(error);
            }
        }
        None
    }

    pub(in crate::executor) fn parameter_assignment_error_in_word(
        &self,
        word: &str,
    ) -> Option<(String, &'static str)> {
        let word = word
            .strip_prefix('\x1b')
            .or_else(|| word.strip_prefix('\x1d'))
            .unwrap_or(word);
        let mut rest = word;
        while let Some(start) = rest.find("${") {
            let after_start = &rest[start + 2..];
            let Some(end) = matching_parameter_brace(after_start) else {
                return None;
            };
            let inner = &after_start[..end];
            if let Some((name, require_non_empty)) = parse_parameter_assignment_operator(inner) {
                if self.parameter_assignment_required(name, require_non_empty) {
                    if name.parse::<usize>().is_ok_and(|index| index > 0) {
                        return Some((format!("${name}"), "cannot assign in this way"));
                    }
                    let target = parse_array_subscript(name)
                        .map(|(array_name, _)| array_name.to_string())
                        .unwrap_or_else(|| {
                            self.nameref_target_name(name)
                                .unwrap_or_else(|| name.to_string())
                        });
                    if is_marked_var(&self.env_vars, READONLY_VARS, &target) {
                        return Some((target, "readonly variable"));
                    }
                }
            } else if let Some((name, require_non_empty)) = parse_special_assignment_operator(inner)
            {
                // GNU subst.c:10404-10410: `=` / `:=` on a special parameter
                // (`!`, `@`, `*`) with the parameter unset (or null for `:=`)
                // reports "$X: cannot assign in this way" and returns
                // &expand_wdesc_error (non-fatal DISCARD, subst.c:4296).
                // #, $, ?, -, 0 are always set, so only !/@/* reach the error
                // path. parse_parameter_assignment_operator rejects these
                // names, and without this branch `${!=x}` fell into the
                // indirect-expansion scanner and misreported "bad
                // substitution" while `${@=x}` expanded silently empty.
                if self.special_assignment_required(name, require_non_empty) {
                    return Some((format!("${name}"), "cannot assign in this way"));
                }
            }
            rest = &after_start[end + 1..];
        }
        None
    }

    pub(in crate::executor) fn parameter_assignment_required(
        &self,
        name: &str,
        require_non_empty: bool,
    ) -> bool {
        match self.parameter_operator_value(name) {
            Some(value) => require_non_empty && value.is_empty(),
            None => true,
        }
    }

    /// GNU fatality condition for `=` / `:=` on a special parameter
    /// (subst.c:10404 path): the error fires only when the parameter is
    /// UNSET (`=`) or unset-or-null (`:=`). `!` is unset until the first
    /// background job; `@` / `*` are unset with zero positional parameters.
    fn special_assignment_required(&self, name: &str, require_non_empty: bool) -> bool {
        let is_set = match name {
            "!" => self.last_background_pid.is_some(),
            "@" | "*" => !self.positional_params.is_empty(),
            _ => true,
        };
        if require_non_empty {
            !is_set
                || self
                    .parameter_error_value(name)
                    .is_none_or(|value| value.is_empty())
        } else {
            !is_set
        }
    }

    pub(in crate::executor) fn parameter_operator_value(&self, name: &str) -> Option<String> {
        if let Some(value) = self.indirect_parameter_operator_value(name) {
            return value;
        }
        // `${arr[*]:-word}` / `${arr[@]:-word}`: operator tests and values
        // apply to the whole array joined with spaces (Bash semantics).
        if let Some(array_name) = name
            .strip_suffix("[*]")
            .or_else(|| name.strip_suffix("[@]"))
        {
            let values = self
                .parameter_array_storage(array_name)
                .map(|storage| array_values(&storage))
                .unwrap_or_default();
            if values.is_empty() {
                return None;
            }
            return Some(values.join(" "));
        }
        if is_shell_name(name) {
            return self
                .dynamic_parameter_value(name)
                .or_else(|| self.shell_variable_value(name));
        }
        if let Some(value) = self.array_element_parameter_value(name) {
            return Some(value);
        }
        self.parameter_error_value(&name)
    }

    fn indirect_parameter_operator_value(&self, name: &str) -> Option<Option<String>> {
        let indirect_name = name.strip_prefix('!')?;
        if let Some(target_name) = self.nameref_target_name(indirect_name) {
            return Some(Some(target_name));
        }
        // GNU param_expand (subst.c:10058-10068) resolves an indirect
        // POSITIONAL reference to the parameter NAME held by that positional,
        // then applies the operator to *that* parameter's set-state and
        // value. With $1=a and a="" (set but null), ${!1-$z} must yield ""
        // (`-` does not substitute for a set parameter) while ${!1:-$z}
        // substitutes $z; with $9 unset the operator sees an unset
        // parameter either way. The previous code fell through to
        // parameter_error_value, which returned $1's raw value ("a") as the
        // operator result.
        if let Ok(index) = indirect_name.parse::<usize>() {
            return match self.positional_params.get(index.saturating_sub(1)) {
                Some(target_name) => Some(self.parameter_operator_value(target_name)),
                None => Some(None),
            };
        }
        let target_expr = self.env_vars.get(indirect_name)?;
        if target_expr.ends_with("[@]") || target_expr.ends_with("[*]") {
            let values = self.indirect_target_values(target_expr);
            if values.is_empty() {
                return Some(None);
            }
            return Some(Some(self.join_expanded_array_values(values, target_expr)));
        }

        Some(self.indirect_target_values(target_expr).into_iter().next())
    }

    fn is_valid_length_parameter_name(name: &str) -> bool {
        if name.is_empty() {
            return true;
        }
        if matches!(name, "@" | "*" | "?" | "$" | "-" | "0") {
            return true;
        }
        if name.parse::<usize>().is_ok() {
            return true;
        }
        if name.ends_with("[@]") || name.ends_with("[*]") {
            let base = &name[..name.len() - 3];
            return !base.is_empty() && is_shell_name(base);
        }
        // GNU subst.c valid_length_expression: after `#`, a leading name
        // character continues as a name; ANY other first character means the
        // `#` itself is the special parameter `$#` and the remainder must be
        // a parameter operator expression with a non-empty word. GNU probe
        // 2026-09-02 (WSL bash 5.2.21): `${#-posparams}` and `${#?:-xyz}`
        // are VALID (`0`), `${#:x}`/`${#:foo}` are valid, while the
        // empty-word operator forms `${#:}`, `${#/}`, `${#%}`, `${#=}`,
        // `${#+}` and the non-name suffixes `${#1xyz}`, `${#x@}`,
        // `${#x:y}` are bad substitution.
        is_shell_name(name) || Self::is_length_operator_expression(name)
    }

    /// Valid indirect target values: shell names, all-digit positionals,
    /// and the special parameters. Anything else triggers GNU's "invalid
    /// variable name" when indirected through (probe: x=123bad).
    fn is_valid_indirect_target(value: &str) -> bool {
        if value.is_empty() {
            return false;
        }
        if is_shell_name(value) || value.chars().all(|c| c.is_ascii_digit()) {
            return true;
        }
        matches!(value, "@" | "*" | "#" | "?" | "$" | "-" | "!")
    }

    /// Validates the body of a `${!...}` expansion: a simple parameter
    /// reference (name, numeric positional, or special), optionally with a
    /// trailing array subscript (`${!arr[@]}` keys form) or a `${!prefix@}`
    /// variable-name listing suffix. `${!}` itself is the $! parameter.
    fn is_valid_indirect_expression(expr: &str) -> bool {
        if expr.is_empty() {
            return true;
        }
        let base = match expr.rfind('[') {
            Some(start) if expr.ends_with(']') => &expr[..start],
            _ => expr,
        };
        if base.is_empty() {
            return false;
        }
        if let Some(prefix) = base.strip_suffix(['@', '*']) {
            return !prefix.is_empty() && is_shell_name(prefix);
        }
        if matches!(base, "@" | "*" | "#" | "?" | "$" | "-" | "!" | "0")
            || base.parse::<usize>().is_ok()
            || is_shell_name(base)
        {
            return true;
        }
        // GNU also accepts a valid leading name followed by an operator
        // expression applied to the indirect value (${!x//c/x}, ${!x:-y},
        // ${!x#pat}, ...; subst.c param_expand). A name followed by a
        // non-operator character (${!bad!}) stays a bad substitution.
        let name_end = base
            .find(|c: char| !(c.is_ascii_alphanumeric() || c == '_'))
            .unwrap_or(base.len());
        if name_end > 0 {
            let head = &base[..name_end];
            // GNU param_expand also applies operators to an indirect
            // POSITIONAL reference: ${!9:-$z} / ${!1-$z} look up $9 / $1 and
            // then apply the default operator to the indirect result
            // (probe: ${!9:-$z} with $9 unset expands to $z's value). The
            // previous is_shell_name-only check rejected these as bad
            // substitution.
            if head.bytes().all(|b| b.is_ascii_digit()) {
                let rest = &base[name_end..];
                return matches!(
                    rest.chars().next(),
                    Some(':' | '-' | '+' | '=' | '?' | '#' | '%' | '/' | '^' | ',' | '@')
                );
            }
            if is_shell_name(head) {
                let rest = &base[name_end..];
                return matches!(
                    rest.chars().next(),
                    Some(':' | '-' | '+' | '=' | '?' | '#' | '%' | '/' | '^' | ',' | '@')
                );
            }
        }
        false
    }
    /// Validates the remainder of a `${#...}` expansion when it does not
    /// start with a shell name: `#` is then the `$#` parameter itself and
    /// the text must be an operator applied to it. Operators with an empty
    /// word (`${#+}`, `${#:}`, `${#/}`) are bad substitution in GNU.
    fn is_length_operator_expression(name: &str) -> bool {
        let mut chars = name.chars();
        let Some(first) = chars.next() else {
            return false;
        };
        if first == ':' {
            let rest = &name[1..];
            if rest.is_empty() {
                return false;
            }
            return match rest.chars().next() {
                Some('-' | '+' | '=' | '?') => rest.len() > 1,
                _ => true,
            };
        }
        matches!(first, '-' | '+' | '=' | '?' | '@' | '^' | ',' | '/' | '%' | '#') && name.len() > 1
    }

    pub(in crate::executor) fn parameter_expansion_error(
        &self,
        cmd: &CommandNode,
    ) -> Option<(String, String, i32)> {
        for word in &cmd.words {
            if let Some(error) = self.parameter_expansion_error_in_word(word) {
                return Some(error);
            }
        }
        for value in cmd.assignment_values() {
            if let Some(error) = self.parameter_expansion_error_in_word(value) {
                return Some(error);
            }
        }

        None
    }

    pub(in crate::executor) fn parameter_expansion_error_in_heredoc_body(
        &self,
        body: &str,
    ) -> Option<(String, String, i32)> {
        if body.starts_with(crate::lexer::QUOTED_HEREDOC_MARKER) {
            return None;
        }
        let body = strip_unterminated_heredoc_marker(strip_quoted_heredoc_marker(body));
        self.parameter_expansion_error_in_word(body)
            .map(|(name, message, status)| (name, message, if status == 127 { 1 } else { status }))
    }

    pub(in crate::executor) fn parameter_expansion_error_in_word(
        &self,
        word: &str,
    ) -> Option<(String, String, i32)> {
        let word = word
            .strip_prefix('\x1b')
            .or_else(|| word.strip_prefix('\x1d'))
            .unwrap_or(word);
        // Preserve Rubash's nested current-shell extension; its `${| ... }`
        // marker disambiguates the legacy enclosing form from this Bash error.
        if word.contains("${|") {
            return None;
        }
        if crate::builtins::set::shell_option_enabled(&self.env_vars, "nounset") {
            if let Some(name) = self.nounset_unbound_parameter(word) {
                return Some((name, "unbound variable".to_string(), 127));
            }
        }
        let mut rest = word;
        while let Some(start) = rest.find("${") {
            let after_start = &rest[start + 2..];
            let Some(end) = matching_parameter_brace(after_start) else {
                return Some((
                    "${".to_string() + after_start + "}",
                    "unexpected EOF while looking for matching `}'".to_string(),
                    2,
                ));
            };
            let inner = &after_start[..end];
            // GNU rejects a parameter expansion nested inside another
            // parameter expansion inside an array subscript. Reject it
            // before the mutable whole-word expander can recurse indefinitely;
            // a single nested subscript such as `${A[${i}]}` remains valid.
            if inner.contains("[${${") {
                return Some((format!("${{{inner}}}"), "bad substitution".to_string(), 1));
            }
            // `${#X}` is the length form. X must be a valid parameter name
            // (special, shell name, numeric positional, or `arr[@]` index form).
            // Other suffixes such as `${#:}`, `${#/}`, `${#1xyz}` are bad
            // substitution in GNU Bash.
            if let Some(length_name) = inner.strip_prefix('#') {
                if !Self::is_valid_length_parameter_name(length_name) {
                    return Some((format!("${{{inner}}}"), "bad substitution".to_string(), 1));
                }
            }
            // Indirect expansions accept a simple parameter reference, an
            // array subscript form, or the prefix@ variable-name listing
            // form; anything else (e.g. a stray trailing '!' in ${!bad!})
            // is a bad substitution in GNU.
            if let Some(indirect) = inner.strip_prefix('!') {
                if !Self::is_valid_indirect_expression(indirect) {
                    return Some((format!("${{{inner}}}"), "bad substitution".to_string(), 1));
                }
                // Value-dependent GNU diagnostics for the plain indirect
                // form ${!name} (subst.c parameter_brace_expand_indir): an
                // unset name reports "invalid indirect expansion" and aborts
                // the command; a set name whose value is not a valid
                // indirect target reports "invalid variable name". A set
                // name whose value is a valid identifier expands normally -
                // even when the target itself is unset (GNU probe:
                // x=validname with validname unset expands empty).
                if !indirect.is_empty() && is_shell_name(indirect) {
                    match self.env_vars.get(indirect) {
                        None => {
                            return Some((
                                indirect.to_string(),
                                "invalid indirect expansion".to_string(),
                                1,
                            ));
                        }
                        Some(value) => {
                            if !Self::is_valid_indirect_target(value) {
                                return Some((
                                    value.clone(),
                                    "invalid variable name".to_string(),
                                    1,
                                ));
                            }
                        }
                    }
                }
            }
            // `${ command; }` is not Bash command substitution. Rubash also
            // supports the distinct `${| command; }` current-shell form, so
            // reject only the whitespace-led form as a bad substitution.
            if inner.chars().next().is_some_and(char::is_whitespace)
                && !word_contains_current_shell_command_substitution(word)
            {
                return Some((format!("${{{inner}}}"), "bad substitution".to_string(), 1));
            }
            if let Some((name, message, require_non_empty)) = parse_parameter_error_operator(inner)
            {
                let value = self.parameter_error_value(name);
                let is_error = if require_non_empty {
                    value.as_deref().map(str::is_empty).unwrap_or(true)
                } else {
                    value.is_none()
                };
                if is_error {
                    let message = if message.is_empty() {
                        if require_non_empty {
                            "parameter null or not set".to_string()
                        } else {
                            "parameter not set".to_string()
                        }
                    } else {
                        self.expand_parameter_word(message)
                    };
                    // Bash reports parameter expansion failures as a
                    // command-not-found-style expansion error (status 127),
                    // including both `?` and `:?` operators.
                    return Some((name.to_string(), message, 127));
                }
            }
            if let Some((name, offset, Some(length))) = self.parse_parameter_substring(inner) {
                if length < 0 {
                    let is_array_slice = name
                        .strip_suffix("[@]")
                        .or_else(|| name.strip_suffix("[*]"))
                        .is_some();
                    let is_invalid = is_array_slice
                        || self.parameter_error_value(name).is_some_and(|value| {
                            parameter_substring_has_negative_result(
                                value.chars().count(),
                                offset,
                                length,
                            )
                        });
                    if is_invalid {
                        return Some((
                            length.to_string(),
                            "substring expression < 0".to_string(),
                            1,
                        ));
                    }
                }
            }
            rest = &after_start[end + 1..];
        }
        None
    }

    pub(in crate::executor) fn nounset_unbound_parameter(&self, word: &str) -> Option<String> {
        let mut chars = word.chars().peekable();
        while let Some(ch) = chars.next() {
            if ch == '\x1f' {
                continue;
            }
            if ch != '$' {
                continue;
            }

            match chars.peek().copied() {
                Some('{') => {
                    chars.next();
                    let mut name = String::new();
                    for name_ch in chars.by_ref() {
                        if name_ch == '}' {
                            break;
                        }
                        name.push(name_ch);
                    }
                    if self.nounset_braced_parameter_is_unbound(&name) {
                        // GNU reports the transform target without the @a/@A
                        // suffix, and a nameref target as the `!ref`
                        // expression (new-exp15: `!bar: unbound variable`).
                        let reported = name
                            .strip_suffix("@a")
                            .or_else(|| name.strip_suffix("@A"))
                            .unwrap_or(&name);
                        return Some(reported.to_string());
                    }
                }
                Some(first) if first.is_ascii_digit() => {
                    chars.next();
                    let index = first.to_digit(10).unwrap_or(0) as usize;
                    if index > 0 && self.positional_params.get(index - 1).is_none() {
                        return Some(format!("${first}"));
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
                    if !self.dynamic_parameter_is_set(&name)
                        && !self.env_vars.contains_key(&name)
                        && std::env::var(&name).is_err()
                    {
                        return Some(name);
                    }
                }
                Some('?') | Some('$') | Some('@') | Some('*') | Some('#') | Some('-') => {
                    chars.next();
                }
                Some('(') => {
                    chars.next();
                }
                Some(_) | None => {}
            }
        }
        None
    }

    pub(in crate::executor) fn nounset_braced_parameter_is_unbound(&self, name: &str) -> bool {
        if name.is_empty()
            || matches!(name, "#" | "@" | "*" | "?" | "$" | "-" | "0")
            || name.starts_with('!')
            || parse_parameter_error_operator(name).is_some()
            || name.contains(":-")
            || name.contains(":=")
            || name.contains(":+")
            || name.contains('-')
            || name.contains('=')
            || name.contains('+')
            || name.contains('#')
            || name.contains('%')
            || name.contains('/')
            || name.contains('^')
            || name.contains(',')
        {
            return false;
        }

        // GNU subst.c parameter_brace_expand: under nounset the attribute
        // transforms ${name@a}/${name@A} report the target as unbound when it
        // has no value, even though the transforms themselves would expand
        // (new-exp15 `-uc` cases; check_unbound_variable precedes
        // string_transform). Run this before the generic `@` bail-out.
        if let Some(stripped) = name.strip_suffix("@a").or_else(|| name.strip_suffix("@A")) {
            let target = stripped.strip_prefix('!').unwrap_or(stripped);
            let resolved = self
                .resolved_variable_name(target)
                .unwrap_or_else(|| target.to_string());
            return !self.dynamic_parameter_is_set(&resolved)
                && !self.env_vars.contains_key(&resolved);
        }

        if name.contains('@') {
            return false;
        }

        if let Ok(index) = name.parse::<usize>() {
            return index > 0 && self.positional_params.get(index - 1).is_none();
        }

        if is_shell_name(name) {
            return !self.dynamic_parameter_is_set(name)
                && !self.env_vars.contains_key(name)
                && std::env::var(name).is_err();
        }

        false
    }

    pub(in crate::executor) fn parameter_error_value(&self, name: &str) -> Option<String> {
        match name {
            "#" => Some(self.positional_params.len().to_string()),
            "@" | "*" => Some(self.positional_params.join(" ")),
            "?" => Some(self.exit_code.to_string()),
            "$" => Some(self.shell_pid_value().to_string()),
            "!" => Some(self.last_background_pid_value()),
            "-" => Some(self.shell_option_flags()),
            "0" => Some(self.script_name_value()),
            _ => {
                if let Some(value) = self.dynamic_parameter_value(name) {
                    return Some(value);
                }
                if let Ok(index) = name.parse::<usize>() {
                    return self.positional_params.get(index.saturating_sub(1)).cloned();
                }
                if let Some(value) = self.array_element_parameter_value(name) {
                    return Some(value);
                }
                self.env_vars.get(name).cloned()
            }
        }
    }
}
