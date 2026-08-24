//! Shared GNU-style parameter-brace scanning primitives.
//!
//! This module owns structural scanning only. It deliberately does not remove
//! shell quotes or expand parameter words.

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum DolbraceState {
    Param,
    Op,
    Word,
    Quote,
    Quote2,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) struct BraceContext {
    pub(crate) outer_double_quote: bool,
    pub(crate) posix: bool,
    pub(crate) replacement_context: bool,
    pub(crate) initial_state: DolbraceState,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum QuoteEventKind {
    Single,
    Double,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) struct QuoteEvent {
    pub(crate) offset: usize,
    pub(crate) kind: QuoteEventKind,
    pub(crate) state: DolbraceState,
}

#[derive(Debug, Eq, PartialEq)]
pub(crate) struct BracedScan {
    pub(crate) end: usize,
    pub(crate) final_state: DolbraceState,
    pub(crate) quote_events: Vec<QuoteEvent>,
}

pub(crate) fn scan_braced_parameter_body(input: &str, options: BraceContext) -> Option<BracedScan> {
    let mut wrapped = String::from("${");
    wrapped.push_str(input);
    let mut scan = scan_braced_parameter(&wrapped, options)?;
    scan.end = scan.end.saturating_sub(2);
    for event in &mut scan.quote_events {
        event.offset = event.offset.saturating_sub(2);
    }
    Some(scan)
}

pub(crate) fn scan_braced_parameter(input: &str, options: BraceContext) -> Option<BracedScan> {
    if !input.starts_with("${") {
        return None;
    }
    let chars: Vec<(usize, char)> = input.char_indices().collect();
    let mut cursor = 2usize;
    let mut depth = 1usize;
    let mut state = options.initial_state;
    let mut states = Vec::new();
    let mut single = false;
    let mut double = false;
    let mut bracket_depth = 0usize;
    let mut quote_events = Vec::new();
    while cursor < chars.len() {
        let (offset, ch) = chars[cursor];
        cursor += 1;
        if ch == '\\' {
            cursor = cursor.saturating_add(1);
            continue;
        }
        if ch == '`' && !single {
            cursor = skip_backtick(&chars, cursor);
            continue;
        }
        if (ch == '<' || ch == '>')
            && chars.get(cursor).is_some_and(|(_, next)| *next == '(')
            && !single
        {
            cursor = skip_parenthesized(&chars, cursor + 1);
            continue;
        }
        if ch == '$' && chars.get(cursor).is_some_and(|(_, next)| *next == '(') && !single {
            let open = if chars.get(cursor + 1).is_some_and(|(_, next)| *next == '(') {
                cursor + 1
            } else {
                cursor
            };
            cursor = skip_parenthesized(&chars, open + 1);
            continue;
        }
        if ch == '$' && chars.get(cursor).is_some_and(|(_, next)| *next == '{') && !single {
            cursor += 1;
            states.push(state);
            depth += 1;
            state = match state {
                DolbraceState::Word | DolbraceState::Quote | DolbraceState::Quote2 => {
                    DolbraceState::Param
                }
                other => other,
            };
            continue;
        }
        if ch == '}'
            && (options.replacement_context || (!single && !double))
            && (bracket_depth == 0 || depth > 1)
        {
            depth -= 1;
            if depth == 0 {
                return Some(BracedScan {
                    end: offset + ch.len_utf8(),
                    final_state: state,
                    quote_events,
                });
            }
            state = states.pop().unwrap_or(DolbraceState::Param);
            continue;
        }
        if ch == '\'' && !double {
            quote_events.push(QuoteEvent {
                offset,
                kind: QuoteEventKind::Single,
                state,
            });
            if (options.outer_double_quote && options.posix)
                || single_quote_has_pair(&chars, cursor)
                || matches!(state, DolbraceState::Quote | DolbraceState::Quote2)
            {
                single = !single;
            }
            continue;
        }
        if ch == '"' && !single {
            quote_events.push(QuoteEvent {
                offset,
                kind: QuoteEventKind::Double,
                state,
            });
            double = !double;
            continue;
        }
        if !single && !double {
            if ch == '[' {
                bracket_depth += 1;
            } else if ch == ']' {
                bracket_depth = bracket_depth.saturating_sub(1);
            }
        }
        let operator = matches!(
            ch,
            '#' | '%' | '^' | ',' | '~' | ':' | '-' | '=' | '?' | '+' | '/'
        );
        match state {
            DolbraceState::Param if operator => {
                state = if matches!(ch, '%' | '#' | '^' | ',') {
                    DolbraceState::Quote
                } else if ch == '/' {
                    DolbraceState::Quote2
                } else {
                    DolbraceState::Op
                }
            }
            DolbraceState::Op if !operator => state = DolbraceState::Word,
            _ => {}
        }
    }
    None
}

fn single_quote_has_pair(chars: &[(usize, char)], mut cursor: usize) -> bool {
    while cursor < chars.len() {
        let ch = chars[cursor].1;
        if ch == '\\' {
            cursor = cursor.saturating_add(2);
            continue;
        }
        if ch == '$' && chars.get(cursor + 1).is_some_and(|(_, next)| *next == '{') {
            return false;
        }
        if ch == '\'' {
            return true;
        }
        cursor += 1;
    }
    false
}

fn skip_backtick(chars: &[(usize, char)], mut cursor: usize) -> usize {
    while cursor < chars.len() {
        if chars[cursor].1 == '\\' {
            cursor = cursor.saturating_add(2);
        } else if chars[cursor].1 == '`' {
            return cursor + 1;
        } else {
            cursor += 1;
        }
    }
    chars.len()
}

fn skip_parenthesized(chars: &[(usize, char)], mut cursor: usize) -> usize {
    let mut depth = 1usize;
    let mut quote = None;
    while cursor < chars.len() {
        let ch = chars[cursor].1;
        cursor += 1;
        if ch == '\\' {
            cursor = cursor.saturating_add(1);
            continue;
        }
        if let Some(active) = quote {
            if ch == active {
                quote = None;
            }
            continue;
        }
        if ch == char::from_u32(39).unwrap() || ch == char::from_u32(34).unwrap() {
            quote = Some(ch);
        } else if ch == '(' {
            depth += 1;
        } else if ch == ')' {
            depth -= 1;
            if depth == 0 {
                return cursor;
            }
        }
    }
    chars.len()
}

#[cfg(test)]
mod tests {
    use super::*;
    const PLAIN: BraceContext = BraceContext {
        outer_double_quote: false,
        posix: false,
        replacement_context: false,
        initial_state: DolbraceState::Param,
    };
    const OUTER_DOUBLE_DEFAULT: BraceContext = BraceContext {
        outer_double_quote: true,
        posix: false,
        replacement_context: false,
        initial_state: DolbraceState::Param,
    };
    const OUTER_DOUBLE_POSIX: BraceContext = BraceContext {
        outer_double_quote: true,
        posix: true,
        replacement_context: false,
        initial_state: DolbraceState::Param,
    };
    const PARAMETER_WORD: BraceContext = BraceContext {
        outer_double_quote: false,
        posix: false,
        replacement_context: false,
        initial_state: DolbraceState::Word,
    };
    const POSIX_DOUBLE_QUOTE_WORD: BraceContext = BraceContext {
        outer_double_quote: true,
        posix: true,
        replacement_context: false,
        initial_state: DolbraceState::Quote,
    };

    #[test]
    fn initial_parameter_word_state_is_preserved() {
        let scan = scan_braced_parameter("${name}", PARAMETER_WORD).unwrap();
        assert_eq!(scan.final_state, DolbraceState::Word);
    }

    #[test]
    fn initial_posix_double_quote_state_records_operator_quotes() {
        let scan = scan_braced_parameter("${IFS+'}'z}", POSIX_DOUBLE_QUOTE_WORD).unwrap();
        assert_eq!(scan.end, "${IFS+'}'z}".len());
        assert_eq!(scan.quote_events.len(), 2);
    }

    #[test]
    fn scans_nested_parameter_and_restores_outer_state() {
        let input = "${A[${i}]}";
        let scan = scan_braced_parameter(input, PLAIN).unwrap();
        assert_eq!(scan.end, input.len());
        assert_eq!(scan.final_state, DolbraceState::Param);
    }
    #[test]
    fn escaped_closing_brace_does_not_close() {
        let input = "${x:-\\}}";
        let scan = scan_braced_parameter(input, PLAIN).unwrap();
        assert_eq!(scan.end, input.len());
    }
    #[test]
    fn ignores_braces_inside_bracket_patterns() {
        let input = "${o%[}]}";
        let scan = scan_braced_parameter(input, PLAIN).unwrap();
        assert_eq!(scan.end, input.len());
    }

    #[test]
    fn skips_opaque_command_and_arithmetic_substitutions() {
        let input = "${x:-$(printf '}') $((1 + 2)) `printf '}'`}";
        let scan = scan_braced_parameter(input, PLAIN).unwrap();
        assert_eq!(scan.end, input.len());
    }

    #[test]
    fn outer_quote_does_not_imply_posix_scanning() {
        let input = "${x:-'}";
        assert!(scan_braced_parameter(input, OUTER_DOUBLE_DEFAULT).is_some());
        assert!(scan_braced_parameter(input, OUTER_DOUBLE_POSIX).is_none());
    }

    #[test]
    fn records_operator_word_quote_metadata() {
        let input = "${IFS+'}'z}";
        let scan = scan_braced_parameter(input, OUTER_DOUBLE_POSIX).unwrap();
        assert_eq!(scan.end, input.len());
        assert_eq!(scan.quote_events.len(), 2);
        assert_eq!(scan.quote_events[0].kind, QuoteEventKind::Single);
        assert_eq!(scan.quote_events[0].state, DolbraceState::Op);
    }
}
