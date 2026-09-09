//! Lexer Module - Bash Tokenizer
//!
//! Transforms raw input strings into tokens for the parser.

mod ansi;
mod brace_scan;
mod classification;
mod continuation;
pub(crate) mod dolbrace;
mod heredoc;
mod heredoc_scan;
mod number_redirect;
mod quotes;
mod scanner;
mod skip;
mod token;
mod word;

#[cfg(test)]
mod tests;

use brace_scan::{has_unclosed_brace_group, opens_function_body_after_previous_signature};
use continuation::{ends_with_unquoted_backslash, has_unclosed_compound_assignment, has_unclosed_quotes};

pub(crate) use continuation::has_unclosed_command_substitution;
use heredoc::heredoc_delimiters;
use scanner::Lexer;

pub(crate) use quotes::remove_shell_quotes;
pub(crate) use quotes::PARAM_NAME_END_MARKER;
pub use token::{Token, TokenKind};

pub(crate) const QUOTED_HEREDOC_MARKER: &str = "__RUBASH_HD1__";

/// Identifies where lexer input came from; alias handling is reserved for later.
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub enum InputOrigin {
    #[default]
    Direct,
    AliasReplacementDeferredHeredoc,
}

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub struct TokenizeOptions {
    pub initial_posix: bool,
    pub input_origin: InputOrigin,
}

pub fn tokenize(input: &str) -> Vec<Token> {
    tokenize_with_options(input, TokenizeOptions::default())
}

pub fn tokenize_with_options(input: &str, options: TokenizeOptions) -> Vec<Token> {
    tokenize_with_initial_posix_and_origin(input, options.initial_posix, options.input_origin)
}

/// Tokenize with an initial POSIX parse mode. GNU Bash parses commands
/// lazily, so a runtime `set -o posix` changes the parse rules only for
/// commands read afterwards. Batch input is tokenized ahead of execution, so
/// the line loop below approximates that by flipping the parse mode when it
/// sees a top-level `set -o posix` / `set +o posix` command.
pub fn tokenize_with_initial_posix(input: &str, posix: bool) -> Vec<Token> {
    tokenize_with_initial_posix_and_origin(input, posix, InputOrigin::Direct)
}

/// Tokenize a substitution body whose text was extracted from a larger
/// script. `start_line` is the 1-based script line where the body begins, so
/// tokens (and the diagnostics they produce) carry original script line
/// numbers instead of restarting at 1 (GNU parse.y keeps in-place line
/// counters for command substitutions).
pub fn tokenize_with_initial_posix_and_line(
    input: &str,
    posix: bool,
    start_line: usize,
) -> Vec<Token> {
    if input.trim().is_empty() {
        return Vec::new();
    }

    let mut tokens = tokenize_with_heredocs(input, posix, InputOrigin::Direct, start_line);
    if tokens
        .last()
        .is_some_and(|token| token.kind == TokenKind::Semicolon)
    {
        tokens.pop();
    }
    tokens
}

pub fn tokenize_with_initial_posix_and_origin(
    input: &str,
    posix: bool,
    input_origin: InputOrigin,
) -> Vec<Token> {
    if input.trim().is_empty() {
        return Vec::new();
    }

    let mut tokens = tokenize_with_heredocs(input, posix, input_origin, 1);
    if tokens
        .last()
        .is_some_and(|token| token.kind == TokenKind::Semicolon)
    {
        tokens.pop();
    }
    tokens
}

fn tokenize_with_heredocs(
    input: &str,
    initial_posix: bool,
    input_origin: InputOrigin,
    start_line: usize,
) -> Vec<Token> {
    // TODO(parse.y/redir.c): Bash parses here-documents after reading the
    // complete command and performs delimiter-specific expansion rules. This
    // line-oriented collector handles the simple `<<word` and `<<'word'`
    // forms used by early upstream alias tests.
    let mut output = Vec::new();
    let mut lines = input.lines();
    let mut position = 0;
    let mut line_number = start_line;
    let mut logical_start_line = start_line;
    let mut logical_line = String::new();
    let mut continued_line = false;
    let mut parse_posix = initial_posix;

    while let Some(line) = lines.next() {
        if logical_line.is_empty() {
            logical_start_line = line_number;
        }
        if !logical_line.is_empty() && !continued_line {
            logical_line.push('\n');
        }
        continued_line = false;
        logical_line.push_str(line);
        position += line.len() + 1;
        let line_had_terminator = position <= input.len();
        line_number += 1;

        if line_had_terminator && ends_with_unquoted_backslash(&logical_line) {
            logical_line.pop();
            continued_line = true;
            continue;
        }

        if has_unclosed_quotes(&logical_line) {
            continue;
        }
        if has_unclosed_command_substitution(&logical_line) {
            continue;
        }
        // A `name=(` compound array assignment keeps reading physical lines
        // until its matching `)` (parse.y; ISSUE #78).
        if has_unclosed_compound_assignment(&logical_line) {
            continue;
        }
        let mut line_tokens = tokenize_plain(&logical_line, parse_posix);
        if let Some(updated) = line_posix_mode_change(&line_tokens) {
            parse_posix = updated;
        }
        // Record the whitespace run before each token. Token::column stays a
        // byte offset into the logical line (only the position field is
        // overwritten with the line number below), so consecutive columns
        // recover the exact inter-token spacing for raw arithmetic capture.
        let mut previous_end = 0usize;
        for token in line_tokens.iter_mut() {
            let start = token.column.min(logical_line.len());
            // Some lexer paths emit tokens whose columns do not advance
            // monotonically through the logical line; skip the gap capture
            // for those instead of slicing an inverted byte range.
            if start >= previous_end {
                let gap = &logical_line[previous_end..start];
                if gap.chars().all(char::is_whitespace) {
                    token.leading_ws = gap.to_string();
                }
            }
            previous_end = previous_end
                .max(start.saturating_add(token.raw.len()))
                .min(logical_line.len());
        }
        let has_heredoc = !heredoc_delimiters(&line_tokens, &logical_line).is_empty();
        if has_unclosed_brace_group(&logical_line)
            && !opens_function_body_after_previous_signature(&logical_line, &output)
            && !has_heredoc
        {
            continue;
        }

        for token in &mut line_tokens {
            token.position = logical_start_line;
        }
        let delimiters = heredoc_delimiters(&line_tokens, &logical_line);
        output.append(&mut line_tokens);
        logical_line.clear();

        for delimiter in delimiters {
            // Alias reparsing must leave the caller's physical input available:
            // its heredoc body belongs to the outer parse, not this replacement.
            if input_origin == InputOrigin::AliasReplacementDeferredHeredoc {
                let body = if delimiter.quoted {
                    QUOTED_HEREDOC_MARKER.to_string()
                } else {
                    String::new()
                };
                output.push(Token::new(TokenKind::HereDocBody, &body, position));
                continue;
            }
            let mut body = String::new();
            let mut continued_body_line = String::new();
            let mut found_delimiter = false;
            for body_line in lines.by_ref() {
                position += body_line.len() + 1;
                line_number += 1;
                let raw_line = body_line.to_string();
                let mut comparable = if delimiter.strip_tabs {
                    raw_line.trim_start_matches('\t').to_string()
                } else {
                    raw_line.clone()
                };

                if !delimiter.quoted {
                    let trailing_slashes =
                        raw_line.chars().rev().take_while(|ch| *ch == '\\').count();
                    if trailing_slashes % 2 == 1 {
                        let mut continued = raw_line;
                        continued.pop();
                        continued_body_line.push_str(&continued);
                        continue;
                    }
                    if !continued_body_line.is_empty() {
                        continued_body_line.push_str(&raw_line);
                        comparable = std::mem::take(&mut continued_body_line);
                    }
                }

                if comparable == delimiter.value
                    || (delimiter.allow_closing_paren
                        && comparable
                            .strip_suffix(')')
                            .is_some_and(|value| value == delimiter.value))
                {
                    found_delimiter = true;
                    break;
                }
                body.push_str(&comparable);
                body.push('\n');
            }
            if !found_delimiter {
                body.insert(0, '\x1f');
            }
            if delimiter.quoted {
                body.insert_str(0, QUOTED_HEREDOC_MARKER);
            }
            output.push(Token::new(TokenKind::HereDocBody, &body, position));
        }
        let mut separator = Token::new(TokenKind::Semicolon, ";", logical_start_line);
        separator.line_break = true;
        output.push(separator);
    }

    if !logical_line.is_empty() {
        let mut line_tokens = tokenize_plain(&logical_line, parse_posix);
        for token in &mut line_tokens {
            token.position = logical_start_line;
        }
        output.append(&mut line_tokens);
        let mut separator = Token::new(TokenKind::Semicolon, ";", logical_start_line);
        separator.line_break = true;
        output.push(separator);
    }

    output
}

pub fn has_unclosed_input_syntax(input: &str) -> bool {
    has_unclosed_quotes(input) || has_unclosed_command_substitution(input)
}

fn tokenize_plain(input: &str, posix: bool) -> Vec<Token> {
    let lexer = Lexer::new(input, posix);
    let mut tokens = Vec::new();
    for token in lexer {
        if token.kind == TokenKind::Eof {
            break;
        }
        tokens.push(token);
    }
    tokens
}

/// Detect top-level `set -o posix` / `set +o posix` commands in a tokenized
/// logical line, returning the POSIX mode that should apply to later lines.
fn line_posix_mode_change(tokens: &[Token]) -> Option<bool> {
    let mut result = None;
    let mut command_start = true;
    let mut index = 0usize;
    while index < tokens.len() {
        let token = &tokens[index];
        let is_separator = token.line_break
            || matches!(
                token.kind,
                TokenKind::Semicolon
                    | TokenKind::And
                    | TokenKind::Or
                    | TokenKind::Background
                    | TokenKind::Pipe
                    | TokenKind::PipeErr
            );
        if is_separator {
            command_start = true;
            index += 1;
            continue;
        }
        if command_start && token.kind == TokenKind::Word && token.value == "set" {
            if let Some(enabled) = set_command_posix_change(&tokens[index + 1..]) {
                result = Some(enabled);
            }
        }
        command_start = false;
        index += 1;
    }
    result
}

fn set_command_posix_change(tokens: &[Token]) -> Option<bool> {
    let mut index = 0usize;
    while index < tokens.len() {
        let token = &tokens[index];
        if token.kind != TokenKind::Word {
            return None;
        }
        let value = token.value.as_str();
        if value == "--" {
            return None;
        }
        if value == "-o" || value == "+o" {
            let next = tokens.get(index + 1)?;
            if next.kind == TokenKind::Word && next.value == "posix" {
                return Some(value == "-o");
            }
            return None;
        }
        if value.starts_with('-') || value.starts_with('+') {
            index += 1;
            continue;
        }
        return None;
    }
    None
}
