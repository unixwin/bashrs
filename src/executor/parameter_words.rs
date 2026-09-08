use super::*;

impl Executor {
    pub(in crate::executor) fn expand_parameter_word(&self, word: &str) -> String {
        // TODO(subst.c/parse.y): The `word` half of ${parameter:-word},
        // ${parameter:=word}, and ${parameter+word} has quote-aware expansion
        // flags. This covers tilde2.tests while the lexer still discards most
        // quote state.
        let expanded = unescape_remaining_shell_escapes(&decode_parameter_word_quotes(
            &self.expand_embedded_parameters(word),
        ));
        tilde_expand::expand_assignment_tilde_value(&expanded, &self.home_value(), false)
    }

    // Alternate word of the `-`/`+`/`:-`/`:+` operators: GNU removes the
    // quotes but keeps whitespace that was quoted or escaped protected from
    // field splitting (posixexp2 37: `${v-a\ b}` is one field `a b`). The
    // `=`/`:=` assignment forms expand to plain data and split (case 35),
    // so they keep using expand_parameter_word.
    //
    // subst.c parameter_brace_expand_rhs (quoted == 0) expands the rhs like
    // any unquoted word: quote removal happens BEFORE parameter expansion,
    // so `\$name` reaches the expander as a protected literal `$` (rhs-exp
    // t33/t34 must NOT expand) and `\p` loses its backslash (t47). The
    // decoder below mirrors the lexer's unquoted-word processing
    // (remove_shell_quotes) with two additions: quoted/escaped whitespace is
    // marked with the \x1c sentinel for the field splitter, and quotes that
    // survive as data inside double-quote regions travel as \x17/\x18
    // sentinels so the expansion walker never re-reads them as live quote
    // structure.
    pub(in crate::executor) fn expand_alternate_parameter_word(&self, word: &str) -> String {
        let decoded = decode_alternate_fragment_unquoted(word);
        let expanded = self.expand_embedded_parameters_protect_ifs(&decoded);
        tilde_expand::expand_assignment_tilde_value(&expanded, &self.home_value(), false)
    }

    pub(in crate::executor) fn expand_parameter_word_mut(&mut self, word: &str) -> String {
        let expanded = unescape_remaining_shell_escapes(&decode_parameter_word_quotes(
            &self.expand_embedded_parameters_mut(word),
        ));
        tilde_expand::expand_assignment_tilde_value(&expanded, &self.home_value(), false)
    }

    pub(in crate::executor) fn expand_quoted_parameter_word(&self, word: &str) -> String {
        // TODO(subst.c/parse.y): Quoted parameter expansion should carry
        // CTLESC/CTLQUOTEMARK state from the parser. This preserves the
        // tilde2.tests distinction that quoted default/alternate words do not
        // perform tilde expansion.
        let Some(name) = word
            .strip_prefix("${")
            .and_then(|word| word.strip_suffix('}'))
        else {
            return self.expand_embedded_parameters(word);
        };
        if !braced_parameter_spans_whole_word(word) {
            return self.expand_embedded_parameters(word);
        }

        if let Some((var_name, default)) = name.split_once(":-") {
            if is_parameter_error_name(var_name) {
                return self
                    .parameter_operator_value(var_name)
                    .filter(|value| !value.is_empty())
                    .map(|value| shell_safe_value(&value))
                    .unwrap_or_else(|| {
                        unescape_parameter_operator_result(
                            &self.expand_embedded_parameters(
                                &decode_double_quotes_in_quoted_parameter_word(default),
                            ),
                            SubstitutionQuoteContext::DoubleQuoted,
                        )
                    });
            }
        }

        if let Some((var_name, alternate)) = name.split_once(":+") {
            if is_parameter_error_name(var_name) {
                if self
                    .parameter_operator_value(var_name)
                    .is_some_and(|value| !value.is_empty())
                {
                    return unescape_parameter_operator_result(
                        &self.expand_embedded_parameters(
                            &decode_double_quotes_in_quoted_parameter_word(alternate),
                        ),
                        SubstitutionQuoteContext::DoubleQuoted,
                    );
                }
                return String::new();
            }
        }

        if let Some((var_name, error_word)) = name.split_once(":?") {
            if is_parameter_error_name(var_name) {
                if self
                    .parameter_operator_value(var_name)
                    .is_some_and(|value| !value.is_empty())
                {
                    return self
                        .parameter_operator_value(var_name)
                        .map(|value| shell_safe_value(&value))
                        .unwrap_or_default();
                }
                return self.expand_embedded_parameters(error_word);
            }
        }

        if let Some((var_name, error_word)) = name.split_once('?') {
            if is_parameter_error_name(var_name) {
                return self
                    .parameter_operator_value(var_name)
                    .map(|value| shell_safe_value(&value))
                    .unwrap_or_else(|| self.expand_embedded_parameters(error_word));
            }
        }

        if let Some((var_name, word)) = name.split_once(":=") {
            if is_parameter_error_name(var_name) {
                return self
                    .parameter_operator_value(var_name)
                    .filter(|value| !value.is_empty())
                    .map(|value| shell_safe_value(&value))
                    .unwrap_or_else(|| self.expand_embedded_parameters(word));
            }
        }

        if let Some((var_name, word)) = name.split_once('=') {
            if is_parameter_error_name(var_name) {
                return self
                    .parameter_operator_value(var_name)
                    .map(|value| shell_safe_value(&value))
                    .unwrap_or_else(|| self.expand_embedded_parameters(word));
            }
        }

        if let Some((var_name, offset, length)) = self.parse_parameter_substring(name) {
            return self.expand_braced_substring_parameter(var_name, offset, length);
        }

        if name.starts_with('#') {
            if let Some(value) = self.expand_braced_indexed_parameter(name) {
                return value;
            }
        }

        if let Some((array_name, _)) = parse_array_subscript(name) {
            // GNU valid_array_reference only treats NAME[...] as a subscript
            // when NAME is a valid identifier; pattern words like `z//[^;]`
            // or `z/#[^;][^;]` must not produce a spurious bad-array-
            // subscript diagnostic here (new-exp8.sub).
            if is_shell_name(array_name) {
                if let Some(value) = self.array_element_parameter_value(name) {
                    return shell_safe_value(&value);
                }
            }
        }

        if let Some(array_name) = name
            .strip_suffix("[@]")
            .or_else(|| name.strip_suffix("[*]"))
            .filter(|array_name| is_shell_name(array_name))
        {
            return self
                .parameter_array_storage(array_name)
                .map(|value| self.join_array_parameter_values(&value, name))
                .unwrap_or_default();
        }

        if let Some((var_name, alternate)) = name.split_once('+') {
            if is_parameter_error_name(var_name) {
                if self.parameter_operator_value(var_name).is_some() {
                    return unescape_parameter_operator_result(
                        &self.expand_embedded_parameters(
                            &decode_double_quotes_in_quoted_parameter_word(alternate),
                        ),
                        SubstitutionQuoteContext::DoubleQuoted,
                    );
                }
                return String::new();
            }
        }

        if let Some((var_name, default)) = name.split_once('-') {
            if is_parameter_error_name(var_name) {
                return self
                    .parameter_operator_value(var_name)
                    .map(|value| shell_safe_value(&value))
                    .unwrap_or_else(|| {
                        unescape_parameter_operator_result(
                            &self.expand_embedded_parameters(
                                &decode_double_quotes_in_quoted_parameter_word(default),
                            ),
                            SubstitutionQuoteContext::DoubleQuoted,
                        )
                    });
            }
        }

        if let Some(value) = self.expand_braced_pattern_or_transform_parameter(name) {
            return value;
        }

        if let Some(value) = self.expand_braced_special_or_indirect_parameter(name) {
            return value;
        }

        self.expand_word(word)
    }

    // GNU param_expand: the word of ${var-word}/${var:=word}/${var+word}/...
    // undergoes tilde expansion only in an unquoted expansion context and only
    // when the word itself starts bare (an explicitly quoted `~` stays
    // literal). Tilde applies at the word start, not after colons.
    fn tilde_expand_operator_word(&self, word: &str, context: SubstitutionQuoteContext) -> String {
        if !matches!(context, SubstitutionQuoteContext::Unquoted) {
            return word.to_string();
        }
        if word.starts_with('"') || word.starts_with('\'') || word.starts_with('\\') {
            return word.to_string();
        }
        tilde_expand::expand_assignment_tilde_value(word, &self.home_value(), false)
    }

    pub(in crate::executor) fn expand_quoted_parameter_word_mut(
        &mut self,
        word: &str,
        context: SubstitutionQuoteContext,
    ) -> String {
        // In POSIX mode, a double-quoted `${...}` may close at a `}` inside
        // the apparent word when a single quote is literal (Interp 221).
        // Expand that braced head separately, then continue with the suffix.
        if matches!(context, SubstitutionQuoteContext::DoubleQuoted)
            && self.posix_mode_enabled()
        {
            if let Some(rest) = word.strip_prefix("${") {
                if let Some(close) = matching_parameter_brace_in_context(rest, true, true) {
                    if close + 1 < rest.len() {
                        let braced_end = 2 + close + 1;
                        let head = self.expand_quoted_parameter_word_mut(&word[..braced_end], context);
                        let tail = self.expand_embedded_parameters_mut_with_context(
                            &word[braced_end..],
                            context,
                        );
                        return format!("{head}{tail}");
                    }
                }
            }
        }

        let Some(name) = word
            .strip_prefix("${")
            .and_then(|word| word.strip_suffix('}'))
        else {
            return self.expand_embedded_parameters_mut_with_context(word, context);
        };
        if !braced_parameter_spans_whole_word(word) {
            return self.expand_embedded_parameters_mut_with_context(word, context);
        }

        if let Some((var_name, default)) = name.split_once(":-") {
            if is_parameter_error_name(var_name) {
                return self
                    .parameter_operator_value(var_name)
                    .filter(|value| !value.is_empty())
                    .map(|value| shell_safe_value(&value))
                    .unwrap_or_else(|| {
                        let default = self.tilde_expand_operator_word(default, context);
                        unescape_parameter_operator_result(
                            &self.expand_embedded_parameters_mut_with_context(
                                &decode_double_quotes_in_quoted_parameter_word(&default),
                                context,
                            ),
                            context,
                        )
                    });
            }
        }

        if let Some((var_name, alternate)) = name.split_once(":+") {
            if is_parameter_error_name(var_name) {
                if self
                    .parameter_operator_value(var_name)
                    .is_some_and(|value| !value.is_empty())
                {
                    let alternate = self.tilde_expand_operator_word(alternate, context);
                    return unescape_parameter_operator_result(
                        &self.expand_embedded_parameters_mut_with_context(
                            &decode_double_quotes_in_quoted_parameter_word(&alternate),
                            context,
                        ),
                        context,
                    );
                }
                return String::new();
            }
        }

        if let Some((var_name, error_word)) = name.split_once(":?") {
            if is_parameter_error_name(var_name) {
                if self
                    .parameter_operator_value(var_name)
                    .is_some_and(|value| !value.is_empty())
                {
                    return self
                        .parameter_operator_value(var_name)
                        .map(|value| shell_safe_value(&value))
                        .unwrap_or_default();
                }
                let error_word = self.tilde_expand_operator_word(error_word, context);
                return self.expand_embedded_parameters_mut_with_context(&error_word, context);
            }
        }

        if let Some((var_name, error_word)) = name.split_once('?') {
            if is_parameter_error_name(var_name) {
                return self
                    .parameter_operator_value(var_name)
                    .map(|value| shell_safe_value(&value))
                    .unwrap_or_else(|| {
                        let error_word = self.tilde_expand_operator_word(error_word, context);
                        self.expand_embedded_parameters_mut_with_context(&error_word, context)
                    });
            }
        }

        if let Some((var_name, word)) = name.split_once(":=") {
            if is_parameter_error_name(var_name) {
                return self
                    .parameter_operator_value(var_name)
                    .filter(|value| !value.is_empty())
                    .map(|value| shell_safe_value(&value))
                    .unwrap_or_else(|| {
                        let word = self.tilde_expand_operator_word(word, context);
                        self.expand_embedded_parameters_mut_with_context(&word, context)
                    });
            }
        }

        if let Some((var_name, word)) = name.split_once('=') {
            if is_parameter_error_name(var_name) {
                return self
                    .parameter_operator_value(var_name)
                    .map(|value| shell_safe_value(&value))
                    .unwrap_or_else(|| {
                        let word = self.tilde_expand_operator_word(word, context);
                        self.expand_embedded_parameters_mut_with_context(&word, context)
                    });
            }
        }

        if let Some((var_name, offset, length)) = self.parse_parameter_substring_mut(name) {
            return self.expand_braced_substring_parameter(var_name, offset, length);
        }

        if name.starts_with('#') {
            if let Some(value) = self.expand_braced_indexed_parameter(name) {
                return value;
            }
        }

        if let Some((array_name, _)) = parse_array_subscript(name) {
            // GNU valid_array_reference only treats NAME[...] as a subscript
            // when NAME is a valid identifier; pattern words like `z//[^;]`
            // or `z/#[^;][^;]` must not produce a spurious bad-array-
            // subscript diagnostic here (new-exp8.sub).
            if is_shell_name(array_name) {
                if let Some(value) = self.array_element_parameter_value(name) {
                    return shell_safe_value(&value);
                }
            }
        }

        if let Some(array_name) = name
            .strip_suffix("[@]")
            .or_else(|| name.strip_suffix("[*]"))
            .filter(|array_name| is_shell_name(array_name))
        {
            return self
                .parameter_array_storage(array_name)
                .map(|value| self.join_array_parameter_values(&value, name))
                .unwrap_or_default();
        }

        if let Some((var_name, alternate)) = name.split_once('+') {
            if is_parameter_error_name(var_name) {
                if self.parameter_operator_value(var_name).is_some() {
                    let alternate = self.tilde_expand_operator_word(alternate, context);
                    let decoded = decode_double_quotes_in_quoted_parameter_word(&alternate);
                    let expanded =
                        self.expand_embedded_parameters_mut_with_context(&decoded, context);
                    let final_value = unescape_parameter_operator_result(&expanded, context);
                    return final_value;
                }
                return String::new();
            }
        }

        if let Some((var_name, default)) = name.split_once('-') {
            if is_parameter_error_name(var_name) {
                return self
                    .parameter_operator_value(var_name)
                    .map(|value| shell_safe_value(&value))
                    .unwrap_or_else(|| {
                        let default = self.tilde_expand_operator_word(default, context);
                        unescape_parameter_operator_result(
                            &self.expand_embedded_parameters_mut_with_context(
                                &decode_double_quotes_in_quoted_parameter_word(&default),
                                context,
                            ),
                            context,
                        )
                    });
            }
        }

        if let Some(value) = self.expand_braced_pattern_or_transform_parameter(name) {
            return value;
        }

        if let Some(value) = self.expand_braced_special_or_indirect_parameter(name) {
            return value;
        }

        self.expand_word(word)
    }

    /// Expand a `=`/`:=` alternate for assignment. Outside double quotes the
    /// alternate is fully quote-removed (`a\ b` -> `a b`, posixexp2 case 35).
    /// Inside double quotes `\` only escapes $, `, ", \, and newline; any
    /// other `\X` is literal data and survives into the assigned value
    /// (`"${v=a\ b}"` assigns `a\ b`, posixexp2 case 36).
    fn expand_assignment_alternate_mut(&mut self, value: &str, double_quoted: bool) -> String {
        if !double_quoted {
            return self.expand_parameter_word_mut(value);
        }
        const PROTECTED_LITERAL_BACKSLASH: char = '\x13';
        let chars: Vec<char> = value.chars().collect();
        let mut protected = String::with_capacity(value.len());
        let mut index = 0usize;
        while index < chars.len() {
            if chars[index] == '\\' {
                if let Some(next) = chars.get(index + 1).copied() {
                    if !matches!(next, '$' | '`' | '"' | '\\' | '\n') {
                        protected.push(PROTECTED_LITERAL_BACKSLASH);
                        protected.push(next);
                        index += 2;
                        continue;
                    }
                }
            }
            protected.push(chars[index]);
            index += 1;
        }
        self.expand_parameter_word_mut(&protected)
            .replace(PROTECTED_LITERAL_BACKSLASH, "\\")
    }

    pub(in crate::executor) fn apply_parameter_assignment_expansions_in_word(
        &mut self,
        word: &str,
    ) {
        // Assignment alternates are quote-removed with the quote rules of
        // the region their `${...}` sits in: inside double quotes `\` only
        // escapes $, `, ", \, and newline, so `"${v=a\ b}"` assigns `a\ b`
        // (posixexp2 case 36), while an unquoted `${v=a\ b}` assigns `a b`
        // and then field-splits (case 35). Parameter bodies inside a
        // command substitution span do NOT inherit the surrounding word's
        // quote context — they are applied later, when the inner command's
        // own words expand (`"x $(printf '%s ' ${v=a\ b})"` assigns `a b`).
        let quoted_word = word.starts_with('\x1d');
        // The prefix scanned for quote/CS context always runs from the word
        // start, so state skipped-over bodies (e.g. inside a command
        // substitution) still counts toward the next body's context.
        let mut consumed = 0usize;
        while let Some(rel) = word[consumed..].find("${") {
            let start = consumed + rel;
            let (in_double, inside_cs) =
                scan_word_prefix_quote_state(&word[..start], quoted_word);
            let body_start = start + 2;
            let Some(end) = matching_parameter_brace(&word[body_start..]) else {
                break;
            };
            if inside_cs {
                // Skip the whole body; the inner expansion applies it.
                consumed = body_start + end + 1;
                continue;
            }
            let inner = &word[body_start..body_start + end];
            self.apply_parameter_assignment_expansion_with_context(inner, in_double);
            consumed = body_start + end + 1;
        }
    }

    fn apply_parameter_assignment_expansion_with_context(
        &mut self,
        inner: &str,
        double_quoted: bool,
    ) {
        if let Some((name, value)) = inner.split_once(":=") {
            if self
                .parameter_operator_value(name)
                .is_some_and(|value| !value.is_empty())
            {
                return;
            }
            let value = self.expand_assignment_alternate_mut(value, double_quoted);
            if self.apply_array_element_parameter_assignment(name, value.clone()) {
                return;
            }
            if self.apply_indirect_parameter_assignment(name, value.clone()) {
                return;
            }
            if !is_shell_name(name) {
                return;
            }
            self.apply_shell_assignment(name, value);
            return;
        }

        if let Some((name, value)) = inner.split_once('=') {
            if self.parameter_operator_value(name).is_some() {
                return;
            }
            let value = self.expand_assignment_alternate_mut(value, double_quoted);
            if self.apply_array_element_parameter_assignment(name, value.clone()) {
                return;
            }
            if self.apply_indirect_parameter_assignment(name, value.clone()) {
                return;
            }
            if !is_shell_name(name) {
                return;
            }
            self.apply_shell_assignment(name, value);
        }
    }

    fn apply_indirect_parameter_assignment(&mut self, name: &str, value: String) -> bool {
        let Some(indirect_name) = name.strip_prefix('!') else {
            return false;
        };
        if self.nameref_target_name(indirect_name).is_some() {
            return false;
        }
        let Some(target_name) = self.env_vars.get(indirect_name).cloned() else {
            return false;
        };
        if self.apply_array_element_parameter_assignment(&target_name, value.clone()) {
            return true;
        }
        if !is_shell_name(&target_name) {
            return false;
        }
        self.apply_shell_assignment(&target_name, value);
        true
    }
}

/// Walk the text before a `${...}` body and report (a) whether the body
/// sits inside double quotes and (b) whether it sits inside a command
/// substitution span. `quoted_word` marks words whose whole text is a
/// double-quoted region (the executor's `\x1d` marker). A `$(...)` span
/// starts a fresh quoting context: quoting inside it does not affect the
/// enclosing region, and it does not inherit the enclosing word's quotes
/// (`"x $(printf '%s ' ${v=a\ b})"` expands the body unquoted).
fn scan_word_prefix_quote_state(prefix: &str, quoted_word: bool) -> (bool, bool) {
    #[derive(Clone, Copy)]
    struct Frame {
        in_single: bool,
        in_double: bool,
    }
    let mut stack: Vec<Frame> = vec![Frame {
        in_single: false,
        in_double: quoted_word,
    }];
    let chars: Vec<char> = prefix.chars().collect();
    let mut index = 0usize;
    while index < chars.len() {
        let frame = *stack.last().expect("quote frame stack is never empty");
        if frame.in_single {
            if chars[index] == '\'' {
                stack.last_mut().unwrap().in_single = false;
            }
            index += 1;
            continue;
        }
        if frame.in_double {
            match chars[index] {
                '\\' => index += 2,
                '"' => stack.last_mut().unwrap().in_double = false,
                '$' if chars.get(index + 1) == Some(&'(') => {
                    stack.push(Frame { in_single: false, in_double: false });
                    index += 2;
                }
                _ => index += 1,
            }
            continue;
        }
        match chars[index] {
            '\'' => {
                stack.last_mut().unwrap().in_single = true;
                index += 1;
            }
            '"' => {
                stack.last_mut().unwrap().in_double = true;
                index += 1;
            }
            '`' => {
                // Old-style substitution: skip to the closing backtick.
                if let Some(close) = chars[index + 1..].iter().position(|c| *c == '`') {
                    index += close + 2;
                    continue;
                }
                break;
            }
            '$' if chars.get(index + 1) == Some(&'(') => {
                stack.push(Frame { in_single: false, in_double: false });
                index += 2;
                continue;
            }
            '\\' => {
                index += 2;
                continue;
            }
            ')' if stack.len() > 1 => {
                stack.pop();
                index += 1;
            }
            _ => {
                index += 1;
            }
        }
    }
    let top = *stack.last().expect("quote frame stack is never empty");
    (top.in_double, stack.len() > 1)
}

fn decode_double_quotes_in_quoted_parameter_word(word: &str) -> String {
    let mut output = String::new();
    let chars = word.chars().collect::<Vec<_>>();
    let mut index = 0usize;
    while index < chars.len() {
        // A backslash escape outside a double-quote span survives quote
        // removal intact: the expansion pass turns it into protected data
        // (`\"` yields a literal quote, posixexp2 case 8). Dropping the
        // escaped quote here made the expansion output an unmarked bare
        // quote that later stages swallowed.
        if chars[index] == '\\'
            && index + 1 < chars.len()
            && matches!(chars[index + 1], '$' | '`' | '"' | '\\' | '}' | '\n')
        {
            output.push(chars[index]);
            output.push(chars[index + 1]);
            index += 2;
            continue;
        }
        if chars[index] != '"' {
            output.push(chars[index]);
            index += 1;
            continue;
        }

        index += 1;
        while index < chars.len() {
            match chars[index] {
                '"' => {
                    index += 1;
                    break;
                }
                '\\' if matches!(chars.get(index + 1), Some('\\' | '"' | '$' | '`' | '\n')) => {
                    index += 1;
                    if index < chars.len() && chars[index] != '\n' {
                        output.push(chars[index]);
                    }
                    index += 1;
                }
                ch => {
                    output.push(ch);
                    index += 1;
                }
            }
        }
    }
    output
}


fn unescape_double_quoted_backslashes(value: &str) -> String {
    let mut output = String::new();
    let mut chars = value.chars().peekable();
    while let Some(ch) = chars.next() {
        if ch == '\\' {
            if let Some(next) = chars.peek().copied() {
                // `}` joins the escapable set because a double-quoted
                // ${...} alternate already lost one escaping level while
                // the body was extracted (subst.c): GNU gives `}z` for
                // `"${IFS+\}z}"` (posixexp2 cases 9/14/15).
                if matches!(next, '$' | '`' | '"' | '\\' | '}' | '\n') {
                    chars.next();
                    if next != '\n' {
                        output.push(next);
                    }
                    continue;
                }
            }
        }
        output.push(ch);
    }
    output
}

// Post-expansion backslash handling for a ${op word} alternate. Quote
// removal already ran on the raw alternate (decode_double_quotes...); the
// expansion result itself is data and only needs its remaining escapes
// resolved (GNU subst.c never quote-removes expansion results).
fn unescape_parameter_operator_result(
    word: &str,
    context: SubstitutionQuoteContext,
) -> String {
    if matches!(context, SubstitutionQuoteContext::DoubleQuoted) {
        unescape_double_quoted_backslashes(word)
    } else {
        unescape_remaining_shell_escapes(word)
    }
}

#[cfg(test)]
mod scanner_tests {
    use super::scan_word_prefix_quote_state;

    #[test]
    fn cs_span_in_plain_word() {
        let word = "A: $(printf '<%s> ' ${w=a\\ b}) | x";
        let start = word.find("${").unwrap();
        let (dq, cs) = scan_word_prefix_quote_state(&word[..start], false);
        assert!(cs, "plain-word CS body must be inside_cs, dq={dq}");
        assert!(!dq);
    }

    #[test]
    fn cs_span_in_quoted_word() {
        let word = "\x1dA: $(printf '<%s> ' ${w=a\\ b}) | x";
        let start = word.find("${").unwrap();
        let (dq, cs) = scan_word_prefix_quote_state(&word[..start], true);
        assert!(cs);
        assert!(!dq, "CS-local quoting, not the outer dquote");
    }

    #[test]
    fn direct_dquote_body() {
        let word = "\x1d${v=a\\ b}";
        let start = word.find("${").unwrap();
        let (dq, cs) = scan_word_prefix_quote_state(&word[..start], true);
        assert!(dq);
        assert!(!cs);
    }

    #[test]
    fn cs_closes_and_next_body_is_outer() {
        let word = "\x1dA: $(f) ${v=a\\ b}";
        let start = word.find("${").unwrap();
        let (dq, cs) = scan_word_prefix_quote_state(&word[..start], true);
        assert!(!cs, "body after the CS span is outer");
        assert!(dq);
    }
}

// Decode an alternate-operator rhs the way GNU expands an unquoted word
// before parameter expansion (subst.c parameter_brace_expand_rhs with
// quoted == 0, reached via expand_string_for_rhs): quotes are removed and
// backslash escapes resolved up front. Mirrors the lexer's
// remove_shell_quotes / remove_double_quoted_into mapping with two
// additions for the field splitter: whitespace that was quoted or escaped
// is marked \x1c so it stays glued to its field (posixexp2 37), and a
// double-quote region's literal single quotes ride as \x17 sentinels so
// the expansion walker does not open a single-quote span on them.
fn decode_alternate_fragment_unquoted(fragment: &str) -> String {
    fn is_protected_whitespace(ch: char) -> bool {
        matches!(ch, ' ' | '\t' | '\n')
    }
    let mut out = String::with_capacity(fragment.len());
    let mut chars = fragment.chars().peekable();
    while let Some(ch) = chars.next() {
        match ch {
            '\'' => {
                // Single-quote region: content is literal data. `$` travels
                // as the protected-dollar marker so the walker does not
                // expand it; quoted whitespace is kept out of field
                // splitting with the \x1c prefix.
                for quoted in chars.by_ref() {
                    if quoted == '\'' {
                        break;
                    }
                    if quoted == '$' {
                        out.push('\x1f');
                    } else if is_protected_whitespace(quoted) {
                        out.push('\x1c');
                        out.push(quoted);
                    } else {
                        out.push(quoted);
                    }
                }
            }
            '"' => {
                // Double-quote region: mirror remove_double_quoted_into.
                while let Some(quoted) = chars.next() {
                    match quoted {
                        '"' => break,
                        '\\' => match chars.peek().copied() {
                            Some('$') => {
                                chars.next();
                                out.push('\x1f');
                            }
                            Some('`') => {
                                chars.next();
                                out.push('\x1a');
                            }
                            Some('\\') => {
                                chars.next();
                                out.push('\x14');
                            }
                            Some('"') => {
                                chars.next();
                                out.push('\x18');
                            }
                            Some('\n') => {
                                chars.next();
                            }
                            _ => out.push('\\'),
                        },
                        '\'' => out.push('\x17'),
                        other if is_protected_whitespace(other) => {
                            out.push('\x1c');
                            out.push(other);
                        }
                        other => out.push(other),
                    }
                }
            }
            '\\' => match chars.next() {
                Some('$') => out.push('\x1f'),
                Some('`') => out.push('\x1a'),
                Some('\'') => out.push('\x17'),
                Some('"') => out.push('\x18'),
                Some('\\') => out.push('\x14'),
                Some(escaped) if is_protected_whitespace(escaped) => {
                    out.push('\x1c');
                    out.push(escaped);
                }
                Some(escaped @ ('*' | '?' | '[' | '@' | '+' | '!')) => {
                    out.push('\x11');
                    out.push(escaped);
                }
                Some(escaped) => out.push(escaped),
                None => out.push('\\'),
            },
            other => out.push(other),
        }
    }
    out
}
