//! Typed command-substitution capture metadata.
//!
//! This is the first owner-scoped boundary corresponding to GNU Bash
//! subst.c::read_comsub. Payload bytes remain data; lexical context is kept
//! separately instead of being encoded in global C0 sentinels.

#[allow(dead_code)]
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(in crate::executor) enum SubstitutionQuoteContext {
    Unquoted,
    DoubleQuoted,
    HereDocument,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub(in crate::executor) struct SubstitutionOutput {
    pub(in crate::executor) bytes: Vec<u8>,
    pub(in crate::executor) status: i32,
    pub(in crate::executor) context: SubstitutionQuoteContext,
}

impl SubstitutionOutput {
    pub(in crate::executor) fn readback(
        mut bytes: Vec<u8>,
        status: i32,
        context: SubstitutionQuoteContext,
    ) -> Self {
        // GNU read_comsub discards NUL bytes while reading the child pipe.
        bytes.retain(|byte| *byte != 0);
        while bytes.last() == Some(&b'\n') {
            bytes.pop();
        }
        Self {
            bytes,
            status,
            context,
        }
    }

    pub(in crate::executor) fn text_lossy(&self) -> String {
        String::from_utf8_lossy(&self.bytes).into_owned()
    }
}

#[allow(dead_code)]
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(in crate::executor) enum SubstitutionSplitPolicy {
    Split,
    NoSplit,
}

#[allow(dead_code)]
#[derive(Clone, Debug, Eq, PartialEq)]
pub(in crate::executor) struct ExpandedFragment {
    pub(in crate::executor) bytes: Vec<u8>,
    pub(in crate::executor) quoted: bool,
    pub(in crate::executor) splittable: bool,
}

impl ExpandedFragment {
    #[allow(dead_code)]
    pub(in crate::executor) fn literal(text: &str, quoted: bool) -> Self {
        Self {
            bytes: text.as_bytes().to_vec(),
            quoted,
            splittable: false,
        }
    }

    #[allow(dead_code)]
    pub(in crate::executor) fn expanded(text: &str, quoted: bool) -> Self {
        Self {
            bytes: text.as_bytes().to_vec(),
            quoted,
            splittable: true,
        }
    }
}

#[allow(dead_code)]
pub(in crate::executor) fn split_expanded_fragments(
    fragments: &[ExpandedFragment],
    ifs: Option<&str>,
    policy: SubstitutionSplitPolicy,
) -> Vec<String> {
    if policy == SubstitutionSplitPolicy::NoSplit {
        return vec![String::from_utf8_lossy(
            &fragments.iter().flat_map(|fragment| fragment.bytes.iter().copied()).collect::<Vec<_>>(),
        )
        .into_owned()];
    }
    let ifs = ifs.unwrap_or(" \t\n");
    let whitespace: Vec<u8> = ifs.bytes().filter(|byte| byte.is_ascii_whitespace()).collect();
    let non_whitespace: Vec<u8> = ifs.bytes().filter(|byte| !byte.is_ascii_whitespace()).collect();
    let mut fields = Vec::new();
    let mut current = String::new();
    let mut saw_unquoted = false;
    let mut pending_non_whitespace = false;
    for fragment in fragments {
        for byte in &fragment.bytes {
            let is_ifs = ifs.as_bytes().contains(byte);
            if fragment.splittable && !fragment.quoted && is_ifs {
                saw_unquoted = true;
                if non_whitespace.contains(byte) {
                    fields.push(std::mem::take(&mut current));
                    pending_non_whitespace = true;
                } else if !current.is_empty() {
                    fields.push(std::mem::take(&mut current));
                    pending_non_whitespace = false;
                }
                continue;
            }
            if !fragment.quoted && whitespace.contains(byte) {
                continue;
            }
            if pending_non_whitespace && current.is_empty() {
                pending_non_whitespace = false;
            }
            current.push(*byte as char);
        }
    }
    if !current.is_empty() || !saw_unquoted {
        fields.push(current);
    }
    fields
}

#[allow(dead_code)]
#[derive(Clone, Debug, Eq, PartialEq)]
pub(in crate::executor) struct SubstitutionSpan {
    pub(in crate::executor) start: usize,
    pub(in crate::executor) end: usize,
    pub(in crate::executor) context: SubstitutionQuoteContext,
}

#[allow(dead_code)]
pub(in crate::executor) fn scan_substitution_spans(raw: &str) -> Vec<SubstitutionSpan> {
    let chars: Vec<(usize, char)> = raw.char_indices().collect();
    let mut spans = Vec::new();
    let mut index = 0usize;
    let mut single = false;
    let mut double = false;
    while index < chars.len() {
        let (offset, ch) = chars[index];
        if ch == '\\' && !single { index += 2; continue; }
        if ch == '\'' && !double { single = !single; index += 1; continue; }
        if ch == '"' && !single { double = !double; index += 1; continue; }
        let dollar_paren = ch == '$'
            && chars.get(index + 1).is_some_and(|(_, next)| *next == '(')
            && chars.get(index + 2).is_none_or(|(_, next)| *next != '(');
        let backtick = ch == char::from(96);
        if !single && (dollar_paren || backtick) {
            let start = offset;
            let context = if double { SubstitutionQuoteContext::DoubleQuoted } else { SubstitutionQuoteContext::Unquoted };
            let mut depth = if dollar_paren { 1usize } else { 0usize };
            let mut cursor = index + if dollar_paren { 2 } else { 1 };
            let mut inner_single = false;
            let mut inner_double = false;
            while cursor < chars.len() {
                let (_, inner) = chars[cursor];
                if inner == '\\' && !inner_single { cursor += 2; continue; }
                if backtick && inner == char::from(96) && !inner_single && !inner_double {
                    spans.push(SubstitutionSpan { start, end: chars[cursor].0 + 1, context });
                    index = cursor + 1; break;
                }
                if dollar_paren {
                    if inner == '\'' && !inner_double { inner_single = !inner_single; }
                    if inner == '"' && !inner_single { inner_double = !inner_double; }
                    if !inner_single && !inner_double && inner == '$' && chars.get(cursor + 1).is_some_and(|(_, next)| *next == '(') { depth += 1; cursor += 2; continue; }
                    if !inner_single && !inner_double && inner == ')' { depth = depth.saturating_sub(1); if depth == 0 { spans.push(SubstitutionSpan { start, end: chars[cursor].0 + 1, context }); index = cursor + 1; break; } }
                }
                cursor += 1;
            }
            if index <= cursor { index = cursor; }
            continue;
        }
        index += 1;
    }
    spans
}

#[allow(dead_code)]
#[derive(Clone, Debug, Eq, PartialEq)]
pub(in crate::executor) struct RawWordFragment {
    pub(in crate::executor) text: String,
    pub(in crate::executor) substitution: bool,
    pub(in crate::executor) context: Option<SubstitutionQuoteContext>,
}

#[allow(dead_code)]
pub(in crate::executor) fn split_raw_word_fragments(raw: &str) -> Vec<RawWordFragment> {
    let spans = scan_substitution_spans(raw);
    if spans.is_empty() {
        return vec![RawWordFragment { text: raw.to_string(), substitution: false, context: None }];
    }
    let mut fragments = Vec::new();
    let mut cursor = 0usize;
    for span in spans {
        if span.start > cursor {
            fragments.push(RawWordFragment { text: raw[cursor..span.start].to_string(), substitution: false, context: None });
        }
        fragments.push(RawWordFragment { text: raw[span.start..span.end].to_string(), substitution: true, context: Some(span.context) });
        cursor = span.end;
    }
    if cursor < raw.len() {
        fragments.push(RawWordFragment { text: raw[cursor..].to_string(), substitution: false, context: None });
    }
    fragments
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn span_scanner_tracks_mixed_quote_contexts() {
        let spans = scan_substitution_spans(r#"pre$(u)"$(q)"post"#);
        assert_eq!(spans.len(), 2);
        assert_eq!(spans[0].context, SubstitutionQuoteContext::Unquoted);
        assert_eq!(spans[1].context, SubstitutionQuoteContext::DoubleQuoted);
    }

    #[test]
    fn span_scanner_ignores_arithmetic_expansions() {
        assert!(scan_substitution_spans("A:$(( )); B:$(printf x)").len() == 1);
        assert_eq!(scan_substitution_spans("A:$(( ))"), Vec::new());
    }

    #[test]
    fn span_scanner_ignores_nested_syntax_inside_single_quotes() {
        let spans = scan_substitution_spans(r#"'$(literal)' $(outer $(inner))"#);
        assert_eq!(spans.len(), 1);
        assert_eq!(spans[0].context, SubstitutionQuoteContext::Unquoted);
    }

    #[test]
    fn raw_fragments_preserve_adjacent_literal_and_substitution_spans() {
        let fragments = split_raw_word_fragments(r#"pre$(u)"$(q)"post"#);
        assert_eq!(fragments.len(), 5);
        assert_eq!(fragments[0].text, "pre");
        assert!(!fragments[0].substitution);
        assert_eq!(fragments[1].text, "$(u)");
        assert_eq!(fragments[1].context, Some(SubstitutionQuoteContext::Unquoted));
        assert_eq!(fragments[2].text, "\"");
        assert_eq!(fragments[3].text, "$(q)");
        assert_eq!(fragments[3].context, Some(SubstitutionQuoteContext::DoubleQuoted));
        assert_eq!(fragments[4].text, "\"post");
    }

    #[test]
    fn readback_removes_nuls_and_only_trailing_newlines() {
        let output = SubstitutionOutput::readback(
            b"a\0b\n\n".to_vec(),
            0,
            SubstitutionQuoteContext::Unquoted,
        );
        assert_eq!(output.bytes, b"ab");
        assert_eq!(output.text_lossy(), "ab");
    }

    #[test]
    fn unquoted_ifs_splits_but_quoted_fragment_does_not() {
        let fragments = [
            ExpandedFragment::expanded("a  b", false),
            ExpandedFragment::literal(" c d", true),
        ];
        assert_eq!(
            split_expanded_fragments(&fragments, Some(" \t\n"), SubstitutionSplitPolicy::Split),
            vec!["a", "b c d"]
        );
    }

    #[test]
    fn non_whitespace_ifs_preserve_interior_empty_fields() {
        let fragments = [ExpandedFragment::expanded("a::b:", false)];
        assert_eq!(
            split_expanded_fragments(&fragments, Some(":"), SubstitutionSplitPolicy::Split),
            vec!["a", "", "b"]
        );
    }

    #[test]
    fn literal_ifs_bytes_are_not_field_split() {
        let fragments = [ExpandedFragment::literal("a::b:", false)];
        assert_eq!(
            split_expanded_fragments(&fragments, Some(":"), SubstitutionSplitPolicy::Split),
            vec!["a::b:"],
        );
    }

    #[test]
    fn no_split_keeps_empty_and_adjacent_fragments_as_one_word() {
        let fragments = [
            ExpandedFragment::literal("pre", true),
            ExpandedFragment::literal("", true),
            ExpandedFragment::literal("post", true),
        ];
        assert_eq!(
            split_expanded_fragments(&fragments, Some(":"), SubstitutionSplitPolicy::NoSplit),
            vec!["prepost"]
        );
    }

    #[test]
    fn readback_keeps_child_quote_bytes_as_data() {
        let output = SubstitutionOutput::readback(
            b"\"x y\"\n".to_vec(),
            0,
            SubstitutionQuoteContext::DoubleQuoted,
        );
        assert_eq!(output.text_lossy(), "\"x y\"");
    }
}
