/// Token types for bash
#[derive(Debug, Clone, PartialEq)]
pub enum TokenKind {
    Word,
    Pipe,
    PipeErr,
    Semicolon,
    RedirectOut,
    RedirectIn,
    Append,
    RedirectErr,
    RedirectErrAppend,
    HereDoc,
    HereString,
    Background,
    And,
    Or,
    Keyword,
    Variable,
    Assignment,
    CommandSubst,
    BraceExpand,
    HereDocBody,
    Eof,
}

/// A single token with its kind, value, and position
#[derive(Debug, Clone)]
pub struct Token {
    pub kind: TokenKind,
    pub value: String,
    pub raw: String,
    pub position: usize,
    pub column: usize,
    /// Whether this separator came from a physical line break.
    pub line_break: bool,
    /// Whitespace immediately before this token in the tokenized source
    /// (parse.y keeps it so arithmetic commands can recover the raw text
    /// between `((` and `))` the way GNU bash's parse_matched_pair does).
    pub leading_ws: String,
}

impl Token {
    pub fn new(kind: TokenKind, value: &str, position: usize) -> Self {
        Self {
            kind,
            value: value.to_string(),
            raw: value.to_string(),
            position,
            column: position,
            line_break: false,
            leading_ws: String::new(),
        }
    }

    pub fn new_with_raw(kind: TokenKind, value: &str, raw: &str, position: usize) -> Self {
        Self {
            kind,
            value: value.to_string(),
            raw: raw.to_string(),
            position,
            column: position,
            line_break: false,
            leading_ws: String::new(),
        }
    }
}
