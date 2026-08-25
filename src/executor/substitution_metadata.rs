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
}

impl ExpandedFragment {
    #[allow(dead_code)]
    pub(in crate::executor) fn literal(text: &str, quoted: bool) -> Self {
        Self {
            bytes: text.as_bytes().to_vec(),
            quoted,
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
            if !fragment.quoted && is_ifs {
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

#[cfg(test)]
mod tests {
    use super::*;

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
            ExpandedFragment::literal("a  b", false),
            ExpandedFragment::literal(" c d", true),
        ];
        assert_eq!(
            split_expanded_fragments(&fragments, Some(" \t\n"), SubstitutionSplitPolicy::Split),
            vec!["a", "b c d"]
        );
    }

    #[test]
    fn non_whitespace_ifs_preserve_interior_empty_fields() {
        let fragments = [ExpandedFragment::literal("a::b:", false)];
        assert_eq!(
            split_expanded_fragments(&fragments, Some(":"), SubstitutionSplitPolicy::Split),
            vec!["a", "", "b"]
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
