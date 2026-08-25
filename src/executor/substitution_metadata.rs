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
    fn readback_keeps_child_quote_bytes_as_data() {
        let output = SubstitutionOutput::readback(
            b"\"x y\"\n".to_vec(),
            0,
            SubstitutionQuoteContext::DoubleQuoted,
        );
        assert_eq!(output.text_lossy(), "\"x y\"");
    }
}
