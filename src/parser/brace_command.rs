use super::*;
use crate::lexer::{Token, TokenKind};

pub(super) fn parse_brace_group_command(
    tokens: &[Token],
    start: usize,
) -> Option<(CommandNode, usize)> {
    let token = tokens.get(start)?;
    if token.kind == TokenKind::Keyword
        && token.value.starts_with('{')
        && token.value.ends_with('}')
        && token.value.len() >= 2
    {
        let inner_source = token.value.trim_start_matches('{').trim_end_matches('}');
        if !brace_group_source_has_completed_command(inner_source) {
            let mut command = CommandNode::new();
            command.assignments.insert(
                "__RUBASH_PARSE_ERROR__".to_string(),
                "unexpected token `}'".to_string(),
            );
            return Some((command, start + 1));
        }
        let inner = inner_source.trim();
        let body_tokens = crate::lexer::tokenize(inner);
        let mut command = CommandNode::new();
        command.line = Some(token.position);
        command.brace_group = Some(Box::new(BraceGroupCommand {
            open_delimiter: "{".to_string(),
            open_delimiter_metadata: delimiter_metadata("{"),
            close_delimiter: "}".to_string(),
            close_delimiter_metadata: delimiter_metadata("}"),
            body: parse(&body_tokens).commands,
        }));
        return Some(finish_compound_command(command, tokens, start + 1));
    }

    if !is_keyword(tokens, start, "{") {
        return None;
    }

    let Some(i) = matching_brace_group_end(tokens, start) else {
        let mut command = CommandNode::new();
        command.assignments.insert(
            "__RUBASH_PARSE_ERROR__".to_string(),
            "unexpected token `}'".to_string(),
        );
        return Some((command, tokens.len()));
    };

    let mut command = CommandNode::new();
    command.line = tokens.get(start).map(|token| token.position);
    command.brace_group = Some(Box::new(BraceGroupCommand {
        open_delimiter: "{".to_string(),
        open_delimiter_metadata: token_metadata(&tokens[start]),
        close_delimiter: "}".to_string(),
        close_delimiter_metadata: token_metadata(&tokens[i]),
        body: parse(&tokens[start + 1..i]).commands,
    }));
    Some(finish_compound_command(command, tokens, i + 1))
}

fn token_metadata(token: &Token) -> Box<WordMetadata> {
    Box::new(build_word_metadata(0, &token.value, &token.raw))
}

fn delimiter_metadata(delimiter: &str) -> Box<WordMetadata> {
    Box::new(build_word_metadata(0, delimiter, delimiter))
}
