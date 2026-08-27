//! Arithmetic expression parsing and evaluation.
//!
//! Provides parsing and evaluation of shell arithmetic expressions including
//! variables, arrays, assignments, and ternary conditionals.

mod parser;

use parser::ConditionalArithParser;
use std::cell::Cell;
use std::collections::HashMap;

use super::Executor;
use crate::executor::{is_marked_var, ASSOC_VARS};

/// Categories surfaced by GNU Bash's arithmetic evaluator.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum ArithmeticErrorCategory {
    EmptyArraySubscript,
    DivisionByZero,
    InvalidLiteral,
    NonVariableAssignment,
    EvaluatorFailure,
}

impl Executor {
    pub(crate) fn eval_arithmetic_command_value(&mut self, expression: &str) -> Option<i128> {
        let expression = if self.has_associative_parameter_subscript(expression) {
            normalize_arithmetic_quotes(expression)
        } else {
            normalize_arithmetic_quotes(&self.expand_arithmetic_special_parameters(expression))
        };
        if crate::builtins::set::shell_option_enabled(&self.env_vars, "nounset") {
            if let Some(name) = arithmetic_unbound_variable(&expression, &self.env_vars) {
                self.env_vars
                    .insert("__RUBASH_ARITH_NOUNSET_ERROR".to_string(), "1".to_string());
                if !self.arithmetic_expansion_error.replace(true) {
                    eprintln!("{}{}: unbound variable", self.diagnostic_prefix(), name);
                }
                return None;
            }
        }
        // In arithmetic command context Bash removes double quotes, but a
        // single-quoted operand is not a numeric literal. Preserve it as an
        // evaluation error so `(( '1' ))` is not silently accepted as 1.
        if expression.contains('\'') {
            return None;
        }
        if empty_quoted_operand_has_operator(&expression) {
            return None;
        }
        // Save a snapshot of variable values before evaluation to detect changes.
        let pre_eval_vars: HashMap<String, String> = self.env_vars.clone();
        let value = eval_mutable_arith_value_with_random(
            &expression,
            &mut self.env_vars,
            Some(&self.random_state),
        );
        self.report_arithmetic_readonly_error();

        // Sync any variable changes from env_vars to shell_state.variables
        // so that subsequent variable expansions see the updated values.
        for (name, new_value) in &self.env_vars {
            if pre_eval_vars.get(name) != Some(new_value) {
                // Variable was modified during arithmetic evaluation
                if let Some(variable) = self.shell_state.variables.get_mut(name) {
                    variable.value = crate::shell::ShellValue::Scalar(new_value.clone());
                } else if !name.starts_with("__RUBASH_") {
                    // Create a new entry in shell_state for non-internal variables
                    let _ = self.shell_state.variables.set_scalar(name, new_value);
                }
            }
        }

        value
    }

    /// Evaluate a `$(( ... ))` expansion embedded in a word. This is the
    /// expansion context: Bash strips double quotes from the expression
    /// before evaluation (`$(( "1" + 1 ))` is `2`), while the command
    /// context (`for (( ... ))` headers) keeps them and rejects them.
    pub(crate) fn eval_arithmetic_expansion_value(&mut self, expression: &str) -> Option<i128> {
        let expression =
            normalize_arithmetic_quotes(&self.expand_arithmetic_special_parameters(expression));
        if crate::builtins::set::shell_option_enabled(&self.env_vars, "nounset") {
            if let Some(name) = arithmetic_unbound_variable(&expression, &self.env_vars) {
                self.env_vars
                    .insert("__RUBASH_ARITH_NOUNSET_ERROR".to_string(), "1".to_string());
                if !self.arithmetic_expansion_error.replace(true) {
                    eprintln!("{}{}: unbound variable", self.diagnostic_prefix(), name);
                }
                return None;
            }
        }
        if empty_quoted_operand_has_operator(&expression) {
            return None;
        }
        // Save a snapshot to detect variable changes from arithmetic side effects
        let pre_eval_vars: HashMap<String, String> = self.env_vars.clone();
        let value = eval_mutable_arith_value_with_random(
            &expression,
            &mut self.env_vars,
            Some(&self.random_state),
        );
        self.report_arithmetic_readonly_error();

        // Sync any variable changes from env_vars to shell_state.variables
        // so that subsequent parameter expansions see arithmetic side effects
        // (e.g., ++i in array subscripts like a[++i]=value).
        for (name, new_value) in &self.env_vars {
            if pre_eval_vars.get(name) != Some(new_value) {
                if let Some(variable) = self.shell_state.variables.get_mut(name) {
                    variable.value = crate::shell::ShellValue::Scalar(new_value.clone());
                } else if !name.starts_with("__RUBASH_") {
                    let _ = self.shell_state.variables.set_scalar(name, new_value);
                }
            }
        }

        value
    }

    fn report_arithmetic_readonly_error(&mut self) {
        let Some(name) = self.env_vars.remove("__RUBASH_ARITH_READONLY_ERROR") else {
            return;
        };
        if !self.arithmetic_expansion_error.replace(true) {
            eprintln!("{}{}: readonly variable", self.diagnostic_prefix(), name);
        }
    }

    fn has_associative_parameter_subscript(&self, expression: &str) -> bool {
        let bytes = expression.as_bytes();
        let mut index = 0;
        while index < bytes.len() {
            if !(bytes[index].is_ascii_alphabetic() || bytes[index] == 95) {
                index += 1;
                continue;
            }
            let start = index;
            index += 1;
            while index < bytes.len()
                && (bytes[index].is_ascii_alphanumeric() || bytes[index] == 95)
            {
                index += 1;
            }
            if index < bytes.len()
                && bytes[index] == 91
                && is_marked_var(&self.env_vars, ASSOC_VARS, &expression[start..index])
                && expression[index..].contains("[$")
            {
                return true;
            }
        }
        false
    }

    pub(super) fn expand_arithmetic_special_parameters(&self, expression: &str) -> String {
        let expression = expression.replace("$#", &self.positional_params.len().to_string());
        self.expand_embedded_parameters(&expression)
    }
}

pub(super) fn eval_arith_value(value: &str) -> i128 {
    value
        .split('+')
        .map(|part| part.trim().parse::<i128>().unwrap_or(0))
        .sum()
}

fn empty_quoted_operand_has_operator(expression: &str) -> bool {
    let chars = expression.chars().collect::<Vec<_>>();
    let mut outside = String::new();
    let mut index = 0;
    let mut found_empty = false;
    while index < chars.len() {
        if chars[index] == '"' {
            let start = index + 1;
            index = start;
            while index < chars.len() && chars[index] != '"' {
                index += 1;
            }
            if index == chars.len() {
                return false;
            }
            if chars[start..index].iter().all(|ch| ch.is_whitespace()) {
                found_empty = true;
            } else {
                outside.extend(chars[start..index].iter().copied());
            }
            index += 1;
        } else {
            outside.push(chars[index]);
            index += 1;
        }
    }
    found_empty
        && outside.chars().any(|ch| {
            matches!(
                ch,
                '+' | '-' | '*' | '/' | '%' | '<' | '>' | '&' | '|' | '^' | '?' | ':'
            )
        })
}

// Word-expansion arithmetic failures abort the enclosing command list:
// top-level lists end the noninteractive run while function bodies stop
// before their remaining commands. GNU Bash 5.2 evidence (2026-08-24):
// $((08)), $((2#2)), $((1/0)), $((1=2)), $((1++)) and $((4 ? 20 : )) all
// print one diagnostic then skip every later command with status 1. A
// `( )` frame ends only the subshell; the caller continues. `let` and
// `(( ))` never reach this predicate: their evaluation failures stay
// nonfatal status-1 continuations (probes d2/d3).
pub(crate) fn arithmetic_expansion_is_fatal(expression: &str) -> bool {
    arithmetic_error_category(expression).is_some()
}

pub(crate) fn arithmetic_error_category(expression: &str) -> Option<ArithmeticErrorCategory> {
    let mut env_vars = HashMap::new();
    let (_, category) = eval_mutable_arith_result(expression, &mut env_vars, None);
    category
}

pub(crate) fn eval_conditional_arith_value(
    value: &str,
    env_vars: &HashMap<String, String>,
) -> Option<i128> {
    let mut env_vars = env_vars.clone();
    eval_mutable_arith_value(value, &mut env_vars)
}

pub(super) fn arithmetic_unbound_variable(
    expression: &str,
    env_vars: &HashMap<String, String>,
) -> Option<String> {
    let mut chars = expression.chars().peekable();
    let mut previous = None;
    while let Some(ch) = chars.next() {
        if !(ch == '_' || ch.is_ascii_alphabetic()) {
            previous = Some(ch);
            continue;
        }
        // Do not mistake digits in hexadecimal or `base#digits` literals for
        // variable names while nounset validation scans the expression.
        if previous.is_some_and(|prev| prev.is_ascii_digit() || prev == '#') {
            while chars
                .peek()
                .is_some_and(|next| next.is_ascii_alphanumeric() || *next == '_')
            {
                previous = chars.next();
            }
            continue;
        }
        let mut name = String::from(ch);
        while chars
            .peek()
            .is_some_and(|next| *next == '_' || next.is_ascii_alphanumeric())
        {
            name.push(chars.next().expect("peeked arithmetic identifier"));
        }
        if !env_vars.contains_key(&name) && !matches!(name.as_str(), "RANDOM" | "SRANDOM") {
            return Some(name);
        }
        previous = name.chars().last();
    }
    // `expr.c::evalexp` marks every parser failure invalid, including
    // malformed array subscripts and adjacent operands that do not fit one
    // of the specialized diagnostics above. Keep the failure observable even
    // when we cannot identify the exact parser token.
    let token = expression
        .split_whitespace()
        .last()
        .filter(|token| !token.is_empty())
        .unwrap_or(expression);
    Some(format!(
        "{expression}: syntax error in expression (error token is \"{token}\")"
    ))
}

/// Strip double quotes from an arithmetic expression before evaluation.
///
/// Bash's expansion context (`$(( ... ))`, `(( ... ))` command, array
/// subscripts, ...) removes double quotes from the expression before the
/// arithmetic evaluator runs: `$(( "1" + 1 ))` is `2` and `$(( "i < 3" ))`
/// evaluates `i < 3`. Single quotes are preserved so the error path can
/// report `operand expected` for `$(( '1' ))` exactly like Bash.
pub(super) fn strip_arith_double_quotes(input: &str) -> String {
    let mut output = String::with_capacity(input.len());
    let bytes = input.as_bytes();
    let mut index = 0;
    while index < bytes.len() {
        let ch = bytes[index];
        if ch != b'"' {
            output.push(ch as char);
            index += 1;
            continue;
        }
        index += 1;
        while index < bytes.len() {
            let next = bytes[index];
            if next == b'"' {
                index += 1;
                break;
            }
            if next == b'\\' {
                index += 1;
                if index < bytes.len() {
                    output.push(bytes[index] as char);
                    index += 1;
                } else {
                    output.push('\\');
                }
            } else {
                output.push(next as char);
                index += 1;
            }
        }
    }
    output
}

fn normalize_arithmetic_quotes(input: &str) -> String {
    let mut output = String::with_capacity(input.len());
    let mut chars = input.chars().peekable();
    while let Some(ch) = chars.next() {
        if ch == '\\' && matches!(chars.peek(), Some('"')) {
            chars.next();
            output.push('"');
        } else {
            output.push(ch);
        }
    }
    output
}

/// Produces a Bash-style error message for an arithmetic expansion that
/// failed to evaluate (`$(( 1.5 ))`, `$(( 2 ** -1 ))`, division by zero, ...).
/// Rubash used to silently drop these; Bash reports them on stderr with rc=1.
pub(in crate::executor) fn arithmetic_error_message(expression: &str) -> Option<String> {
    if let Some(token) = arithmetic_division_by_zero_token(expression) {
        return Some(format!(
            "{expression}: division by 0 (error token is \"{token}\")"
        ));
    }

    if let Some((token, error)) = invalid_based_literal(expression) {
        return Some(format!(
            "{expression}: {error} (error token is \"{token}\")"
        ));
    }

    // GNU Bash rejects a bare assignment target behind && / || even when
    // short-circuit skips it: `$((0 && B=42))` fails with
    // "attempted assignment to non-variable" (error token is "=42").
    if let Some(token) = logical_rhs_assignment_token(expression) {
        return Some(format!(
            "{expression}: attempted assignment to non-variable (error token is \"{token}\")"
        ));
    }

    // An empty ternary branch is a parse failure in Bash:
    // `$((4 ? 20 : ))` reports "expression expected" (error token is ": ").
    if let Some(token) = empty_ternary_branch_token(expression) {
        return Some(format!(
            "{expression}: expression expected (error token is \"{token}\")"
        ));
    }

    // Bash rejects numeric constants as assignment or increment lvalues.
    // The evaluator reports this as a failed expression; preserve the useful
    // diagnostic instead of silently returning status 1.
    let trimmed = expression.trim();
    if trimmed == "++" || trimmed == "--" {
        let token = if trimmed == "++" { "+ " } else { "- " };
        let display_expression = format!("{trimmed} ");
        return Some(format!(
            "{display_expression}: syntax error: operand expected (error token is \"{token}\")"
        ));
    }
    if trimmed
        .split_once('=')
        .is_some_and(|(left, _)| left.trim().chars().all(|ch| ch.is_ascii_digit()))
        || trimmed
            .strip_suffix("++")
            .or_else(|| trimmed.strip_suffix("--"))
            .is_some_and(|value| value.trim().chars().all(|ch| ch.is_ascii_digit()))
    {
        let message = if trimmed.split_once('=').is_some() {
            "attempted assignment to non-variable"
        } else {
            "syntax error: operand expected"
        };
        return Some(format!(
            "{expression}: {message} (error token is \"{}\")",
            trimmed.trim_start_matches(|ch: char| ch.is_ascii_digit())
        ));
    }

    if empty_quoted_operand_has_operator(expression) {
        return Some(format!(
            "{expression}: syntax error: operand expected (error token is \"\"\")"
        ));
    }

    if trimmed.ends_with(['+', '-', '*', '/', '%', '&', '|', '^', '<', '>']) {
        let token = trimmed.chars().last().unwrap_or_default();
        return Some(format!(
            "{expression}: syntax error: operand expected (error token is \"{token}\")"
        ));
    }

    let bytes = expression.as_bytes();
    for index in 0..bytes.len() {
        // Floating point like `1.5`: digit followed by `.digit`.
        if bytes[index].is_ascii_digit()
            && bytes.get(index + 1) == Some(&b'.')
            && bytes
                .get(index + 2)
                .is_some_and(|byte| byte.is_ascii_digit())
        {
            let mut end = index + 1;
            while bytes
                .get(end)
                .is_some_and(|byte| byte.is_ascii_digit() || *byte == b'.')
            {
                end += 1;
            }
            let token = &expression[index + 1..end];
            return Some(format!(
                "{expression}: syntax error: invalid arithmetic operator (error token is \"{token} \")"
            ));
        }
    }

    if let Some(index) = expression.find("**") {
        let after = expression[index + 2..].trim_start();
        if let Some(digits) = after
            .strip_prefix('-')
            .map(|rest| {
                rest.chars()
                    .take_while(|ch| ch.is_ascii_digit())
                    .collect::<String>()
            })
            .filter(|digits| !digits.is_empty())
        {
            return Some(format!(
                "{expression}: exponent less than 0 (error token is \"{digits} \")"
            ));
        }
    }

    // Quoted operand like `$(( '1' ))`: Bash treats `'1'` as a variable
    // reference (which does not exist) and reports `operand expected`.
    // Double quotes are fine (`$(( "1" ))` is 1), so only single quotes count.
    if let Some(start) = expression.find('\'') {
        let rest = &expression[start + 1..];
        let end = rest
            .find('\'')
            .map(|index| start + 1 + index)
            .unwrap_or(expression.len());
        let token = &expression[start..end];
        return Some(format!(
            "{expression}: syntax error: operand expected (error token is \"{token} \")"
        ));
    }

    if expression.contains('?') && expression.contains(':') && expression.contains('=') {
        let branch = expression
            .split_once(':')
            .map(|(_, branch)| branch)
            .unwrap_or("");
        let equals = branch.find('=').unwrap_or(0);
        let token_start = equals
            .checked_sub(1)
            .filter(|index| "+-*/%&^|<>".contains(branch.as_bytes()[*index] as char))
            .unwrap_or(equals);
        let token = branch[token_start..].trim();
        return Some(format!(
            "{expression}: attempted assignment to non-variable (error token is \"{token}\")"
        ));
    }

    None
}

fn invalid_octal_literal(expression: &str) -> Option<String> {
    let bytes = expression.as_bytes();
    let mut index = 0;
    while index < bytes.len() {
        if !bytes[index].is_ascii_digit()
            || (index > 0 && (bytes[index - 1].is_ascii_alphanumeric() || bytes[index - 1] == b'_'))
        {
            index += 1;
            continue;
        }

        let start = index;
        while bytes.get(index).is_some_and(|byte| byte.is_ascii_digit()) {
            index += 1;
        }
        let token = &expression[start..index];
        if token.len() > 1 && token.starts_with('0') && token.bytes().any(|byte| byte >= b'8') {
            return Some(token.to_string());
        }
    }
    None
}

#[derive(Clone, Copy)]
enum ArithmeticLiteralError {
    InvalidBase,
    InvalidIntegerConstant,
    ValueTooGreatForBase,
    InvalidNumber,
}

impl ArithmeticLiteralError {
    fn message(self) -> &'static str {
        match self {
            Self::InvalidBase => "invalid arithmetic base",
            Self::InvalidIntegerConstant => "invalid integer constant",
            Self::ValueTooGreatForBase => "value too great for base",
            Self::InvalidNumber => "invalid number",
        }
    }
}

fn invalid_based_literal(expression: &str) -> Option<(String, &'static str)> {
    let bytes = expression.as_bytes();
    let mut index = 0;
    while index < bytes.len() {
        if !bytes[index].is_ascii_digit()
            || (index > 0 && (bytes[index - 1].is_ascii_alphanumeric() || bytes[index - 1] == b'_'))
        {
            index += 1;
            continue;
        }

        let start = index;
        while bytes.get(index).is_some_and(|byte| byte.is_ascii_digit()) {
            index += 1;
        }
        if bytes.get(index) != Some(&b'#') {
            continue;
        }
        index += 1;
        let digits_start = index;
        while bytes
            .get(index)
            .is_some_and(|byte| byte.is_ascii_alphanumeric() || matches!(byte, b'@' | b'_'))
        {
            index += 1;
        }
        let mut token_end = index;
        while bytes.get(token_end) == Some(&b'#') {
            token_end += 1;
            while bytes
                .get(token_end)
                .is_some_and(|byte| byte.is_ascii_alphanumeric() || matches!(byte, b'@' | b'_'))
            {
                token_end += 1;
            }
        }
        let token = &expression[start..token_end];
        let base = expression[start..digits_start - 1].parse::<u32>().ok();
        let digits = &expression[digits_start..index];
        let error = if token_end != index {
            ArithmeticLiteralError::InvalidNumber
        } else if base == Some(0) {
            ArithmeticLiteralError::InvalidNumber
        } else if base.is_none() || !base.is_some_and(|base| (2..=64).contains(&base)) {
            ArithmeticLiteralError::InvalidBase
        } else if digits.is_empty() {
            ArithmeticLiteralError::InvalidIntegerConstant
        } else if !digits.chars().all(|digit| {
            arithmetic_digit_value(digit, base.unwrap()).is_some_and(|value| value < base.unwrap())
        }) {
            ArithmeticLiteralError::ValueTooGreatForBase
        } else {
            continue;
        };
        return Some((token.to_string(), error.message()));
    }
    invalid_octal_literal(expression).map(|token| {
        (
            token,
            ArithmeticLiteralError::ValueTooGreatForBase.message(),
        )
    })
}

pub(super) fn arithmetic_division_by_zero_token(expression: &str) -> Option<&'static str> {
    let bytes = expression.as_bytes();
    let mut index = 0;
    while index < bytes.len() {
        if !matches!(bytes[index], b'/' | b'%') {
            index += 1;
            continue;
        }
        index += 1;
        while bytes
            .get(index)
            .is_some_and(|byte| byte.is_ascii_whitespace())
        {
            index += 1;
        }
        if matches!(bytes.get(index), Some(b'+' | b'-')) {
            index += 1;
        }
        let start = index;
        while bytes.get(index).is_some_and(|byte| byte.is_ascii_digit()) {
            index += 1;
        }
        if start != index
            && expression[start..index]
                .parse::<i128>()
                .is_ok_and(|value| value == 0)
        {
            return Some("0 ");
        }
    }
    None
}

fn eval_mutable_arith_value(value: &str, env_vars: &mut HashMap<String, String>) -> Option<i128> {
    eval_mutable_arith_value_with_random(value, env_vars, None)
}

pub(super) fn eval_mutable_arith_value_with_random(
    value: &str,
    env_vars: &mut HashMap<String, String>,
    random_state: Option<&Cell<u32>>,
) -> Option<i128> {
    // GNU Bash's subexpr() treats an empty arithmetic expression as zero.
    // This matters for expansion and variable contexts, where an empty
    // quoted operand is valid rather than a parser failure. Lexer quote
    // markers must be normalized before lvalue parsing as well as expansion.
    let normalized = normalize_arithmetic_quotes(value);
    if normalized.trim().is_empty() {
        return Some(0);
    }
    eval_mutable_arith_result(value, env_vars, random_state).0
}

fn eval_mutable_arith_result(
    value: &str,
    env_vars: &mut HashMap<String, String>,
    random_state: Option<&Cell<u32>>,
) -> (Option<i128>, Option<ArithmeticErrorCategory>) {
    let normalized = normalize_arithmetic_quotes(value);
    if normalized.trim().is_empty() {
        return (Some(0), None);
    }
    let mut parser = ConditionalArithParser {
        input: normalized.as_bytes(),
        pos: 0,
        env_vars,
        resolving: Vec::new(),
        random_state,
        error_category: None,
    };
    let value = parser.parse_comma();
    parser.skip_ws();
    let value = value.filter(|_| parser.pos == parser.input.len());
    let category = parser.error_category.or_else(|| {
        if value.is_none() && numeric_assignment_expression(&normalized) {
            Some(ArithmeticErrorCategory::NonVariableAssignment)
        } else if value.is_none() {
            Some(ArithmeticErrorCategory::EvaluatorFailure)
        } else {
            None
        }
    });
    (value, category)
}

fn numeric_assignment_expression(expression: &str) -> bool {
    let Some((left, _)) = expression.split_once('=') else {
        return false;
    };
    !left.trim().is_empty() && left.trim().chars().all(|ch| ch.is_ascii_digit())
}

fn bash_arith(value: i128) -> i128 {
    value as i64 as i128
}

fn checked_arithmetic_pow(base: i128, exponent: i128) -> Option<i128> {
    let exponent = u32::try_from(exponent).ok()?;
    let mut value = 1i128;
    for _ in 0..exponent {
        value = bash_arith(value * base);
    }
    Some(value)
}

fn parse_arithmetic_digits(digits: &[u8], base: u32) -> Option<i128> {
    let mut value = 0i128;
    for digit in std::str::from_utf8(digits).ok()?.chars() {
        let digit = arithmetic_digit_value(digit, base)?;
        if digit >= base {
            return None;
        }
        value = bash_arith(value * i128::from(base) + i128::from(digit));
    }
    Some(value)
}

fn arithmetic_digit_value(ch: char, base: u32) -> Option<u32> {
    match ch {
        '0'..='9' => Some(ch as u32 - '0' as u32),
        'a'..='z' => Some(10 + ch as u32 - 'a' as u32),
        'A'..='Z' if base <= 36 => Some(10 + ch as u32 - 'A' as u32),
        'A'..='Z' => Some(36 + ch as u32 - 'A' as u32),
        '@' => Some(62),
        '_' => Some(63),
        _ => None,
    }
}

fn skip_arith_ws(input: &[u8], pos: &mut usize) {
    while input.get(*pos).is_some_and(|ch| ch.is_ascii_whitespace()) {
        *pos += 1;
    }
}

fn assignment_operator_at(input: &[u8], pos: usize) -> Option<&'static str> {
    for op in [
        "<<=", ">>=", "**=", "+=", "-=", "*=", "/=", "%=", "&=", "^=", "|=", "=",
    ] {
        if op == "="
            && (input.get(pos + 1) == Some(&b'=')
                || (pos > 0 && matches!(input.get(pos - 1), Some(b'!') | Some(b'<') | Some(b'>'))))
        {
            continue;
        }
        if input
            .get(pos..)
            .is_some_and(|rest| rest.starts_with(op.as_bytes()))
        {
            return Some(op);
        }
    }
    None
}

#[cfg(test)]
mod fatality_tests {
    use super::arithmetic_expansion_is_fatal;

    #[test]
    fn invalid_literals_are_fatal_arithmetic_expansion_errors() {
        assert!(arithmetic_expansion_is_fatal("08"));
        assert!(arithmetic_expansion_is_fatal("2#2"));
    }

    /// Word-expansion probe evidence (GNU Bash 5.2.37, 2026-08-24):
    /// `$((1/0)); echo after` never reaches `after`, status 1. Only
    /// command-context evaluation (`let`, `(( ))`) keeps running.
    #[test]
    fn division_by_zero_in_word_expansion_is_fatal() {
        assert!(arithmetic_expansion_is_fatal("1/0"));
    }
}

/// Detects `&& B=...` / `|| B=...` shapes whose right-hand side starts with
/// an identifier assignment. Returns the operator-prefixed error token
/// (for example `"=42"` for `0 && B=42`), mirroring GNU expr.c diagnostics.
fn logical_rhs_assignment_token(expression: &str) -> Option<String> {
    for op in ["&&", "||"] {
        let mut from = 0;
        while let Some(at) = expression[from..].find(op) {
            let rest = expression[from + at + op.len()..].trim_start();
            let first = match rest.chars().next() {
                Some(ch) => ch,
                None => return None,
            };
            if !(first.is_ascii_alphabetic() || first == '_') {
                from += at + op.len();
                continue;
            }
            let mut len = first.len_utf8();
            while rest[len..]
                .chars()
                .next()
                .is_some_and(|ch| ch.is_ascii_alphanumeric() || ch == '_')
            {
                len += rest[len..].chars().next().unwrap().len_utf8();
            }
            let after_trimmed = rest[len..].trim_start();
            if !after_trimmed.starts_with('=') {
                return None;
            }
            let operand: String = after_trimmed[1..]
                .trim_start()
                .chars()
                .take_while(|ch| !matches!(ch, ' ' | '\t' | ')' | ',' | ';'))
                .collect();
            return Some(format!("={operand}"));
        }
    }
    None
}

/// Detects `$((cond ? true :))` shapes where the false branch holds only
/// whitespace; returns the `": "` error token GNU prints.
fn empty_ternary_branch_token(expression: &str) -> Option<String> {
    let question = expression.find('?')?;
    let colon = expression[question..].find(':')? + question;
    if expression[colon + 1..].trim().is_empty() {
        Some(": ".to_string())
    } else {
        None
    }
}
