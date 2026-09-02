//! Brace expansion for {a,b,c} comma-separated lists and {1..5}/{a..e} sequences.
//!
//! GNU Bash source ownership:
// - braces.c

/// Expand brace patterns in a word, returning multiple words.
/// Handles {a,b,c} comma-separated lists and {1..5}, {a..e} sequences.
/// Returns a single-element vec if no braces found (no expansion needed).
pub fn expand_braces(word: &str) -> Vec<String> {
    let mut result = vec![word.to_string()];
    let mut changed = true;
    while changed {
        changed = false;
        let mut new_result = Vec::new();
        for w in &result {
            if let Some(expanded) = expand_single_brace(w) {
                new_result.extend(expanded);
                changed = true;
            } else {
                new_result.push(w.clone());
            }
        }
        result = new_result;
    }
    result
}

fn expand_single_brace(s: &str) -> Option<Vec<String>> {
    let bytes = s.as_bytes();
    let mut i = 0;
    let mut escaped = false;

    while i < bytes.len() {
        if escaped {
            escaped = false;
            i += 1;
            continue;
        }
        if bytes[i] == b'\\' {
            escaped = true;
            i += 1;
            continue;
        }
        // Skip single-quoted strings
        if bytes[i] == b'\'' {
            i += 1;
            while i < bytes.len() && bytes[i] != b'\'' {
                if bytes[i] == b'\\' {
                    i += 1;
                }
                i += 1;
            }
            i += 1;
            continue;
        }
        // Skip double-quoted strings
        if bytes[i] == b'"' {
            i += 1;
            while i < bytes.len() && bytes[i] != b'"' {
                if bytes[i] == b'\\' {
                    i += 1;
                }
                i += 1;
            }
            i += 1;
            continue;
        }
        if bytes[i] != b'{' {
            i += 1;
            continue;
        }
        // Bash ignores a brace opener at a word boundary when it is followed
        // by whitespace or a closing brace; this keeps `{ a,b}` literal.
        let preceded_by_whitespace = i == 0 || bytes[i - 1].is_ascii_whitespace();
        let followed_by_whitespace_or_close = i + 1 >= bytes.len()
            || bytes[i + 1].is_ascii_whitespace()
            || bytes[i + 1] == b'}';
        if preceded_by_whitespace && followed_by_whitespace_or_close {
            i += 1;
            continue;
        }
        // Skip ${...} parameter expansions
        if i > 0 && bytes[i - 1] == b'$' {
            i += 1;
            continue;
        }

        let prefix = &s[..i];
        let inner_start = i + 1;
        let mut depth = 1u32;
        let mut j = inner_start;
        let mut has_comma = false;
        let mut has_double_dot = false;
        let mut j_escaped = false;

        while j < bytes.len() && depth > 0 {
            if j_escaped {
                j_escaped = false;
                j += 1;
                continue;
            }
            match bytes[j] {
                b'\\' => {
                    j_escaped = true;
                    j += 1;
                    continue;
                }
                b'{' => depth += 1,
                b'}' => {
                    depth -= 1;
                    if depth == 0 {
                        break;
                    }
                }
                b',' if depth == 1 => has_comma = true,
                b'.' if depth == 1 && j + 1 < bytes.len() && bytes[j + 1] == b'.' => {
                    has_double_dot = true;
                }
                _ => {}
            }
            j += 1;
        }

        if depth != 0 {
            i += 1;
            continue;
        }

        let inner = &s[inner_start..j];
        let suffix = &s[j + 1..];

        if has_comma {
            let items: Vec<&str> = split_brace_commas(inner);
            if items.len() >= 2 {
                let mut out = Vec::new();
                for item in items {
                    out.push(format!("{prefix}{item}{suffix}"));
                }
                return Some(out);
            }
        } else if has_double_dot {
            if let Some(items) = expand_range(inner) {
                let mut out = Vec::new();
                for item in items {
                    out.push(format!("{prefix}{item}{suffix}"));
                }
                return Some(out);
            }
            // GNU removes an invalid outer sequence wrapper, but still expands
            // nested comma groups. Nested sequence groups remain literal.
            if inner.contains('{') {
                let nested = expand_nested_commas(inner);
                if nested.len() > 1 || nested.first().is_some_and(|item| item != inner) {
                    return Some(
                        nested
                            .into_iter()
                            .map(|item| format!("{prefix}{item}{suffix}"))
                            .collect(),
                    );
                }
            }
        }

        i += 1;
    }
    None
}

fn expand_nested_commas(s: &str) -> Vec<String> {
    let bytes = s.as_bytes();
    let mut i = 0;
    while i < bytes.len() {
        if bytes[i] != b'{' { i += 1; continue; }
        let start = i;
        let mut depth = 1u32;
        let mut j = i + 1;
        while j < bytes.len() && depth > 0 {
            match bytes[j] {
                b'\\' => j += 1,
                b'{' => depth += 1,
                b'}' => depth -= 1,
                _ => {}
            }
            j += 1;
        }
        if depth != 0 { break; }
        let inner = &s[start + 1..j - 1];
        let items = split_brace_commas(inner);
        if items.len() >= 2 {
            let prefix = &s[..start];
            let suffix = &s[j..];
            let mut out = Vec::new();
            for item in items {
                for expanded in expand_nested_commas(item) {
                    out.push(format!("{prefix}{expanded}{suffix}"));
                }
            }
            return out;
        }
        i = j;
    }
    vec![s.to_string()]
}

fn split_brace_commas(s: &str) -> Vec<&str> {
    let mut parts = Vec::new();
    let mut depth = 0u32;
    let mut start = 0;
    let bytes = s.as_bytes();
    let mut i = 0;
    let mut escaped = false;
    while i < bytes.len() {
        if escaped {
            escaped = false;
            i += 1;
            continue;
        }
        match bytes[i] {
            b'\\' => {
                escaped = true;
                i += 1;
                continue;
            }
            b'{' => depth += 1,
            b'}' => depth = depth.saturating_sub(1),
            b',' if depth == 0 => {
                parts.push(&s[start..i]);
                start = i + 1;
            }
            _ => {}
        }
        i += 1;
    }
    parts.push(&s[start..]);
    parts
}

fn expand_range(s: &str) -> Option<Vec<String>> {
    let parts = s.split("..").collect::<Vec<_>>();
    let ([left, right] | [left, right, _]) = parts.as_slice() else {
        return None;
    };
    if left.is_empty() || right.is_empty() {
        return None;
    }
    let step = match parts.as_slice() {
        [_, _] => 1,
        [_, _, step] => step.parse::<i64>().ok()?.abs().max(1),
        _ => return None,
    };

    // Numeric range
    if let (Ok(start), Ok(end)) = (left.parse::<i64>(), right.parse::<i64>()) {
        let width = numeric_range_width(left, right);
        let step = if start <= end { step } else { -step };
        let mut result = Vec::new();
        let mut current = start;
        while (step > 0 && current <= end) || (step < 0 && current >= end) {
            result.push(format_numeric_range_value(current, width));
            current += step;
        }
        return Some(result);
    }
    // Alpha range
    let start = left.as_bytes()[0];
    let end = right.as_bytes()[0];
    if left.len() == 1
        && right.len() == 1
        && start.is_ascii_alphabetic()
        && end.is_ascii_alphabetic()
    {
        let step = i16::try_from(step).ok()?;
        let step: i16 = if start <= end { step } else { -step };
        let mut result = Vec::new();
        let mut current = start as i16;
        while (step > 0 && current <= end as i16) || (step < 0 && current >= end as i16) {
            result.push((current as u8 as char).to_string());
            current += step;
        }
        return Some(result);
    }
    None
}

fn numeric_range_width(left: &str, right: &str) -> Option<usize> {
    let left_digits = left.trim_start_matches('-');
    let right_digits = right.trim_start_matches('-');
    let padded = [left_digits, right_digits]
        .iter()
        .any(|value| value.len() > 1 && value.starts_with('0'));
    padded.then(|| left.len().max(right.len()))
}

fn format_numeric_range_value(value: i64, width: Option<usize>) -> String {
    let Some(width) = width else {
        return value.to_string();
    };
    if value < 0 {
        format!(
            "-{:0width$}",
            value.unsigned_abs(),
            width = width.saturating_sub(1)
        )
    } else {
        format!("{value:0width$}")
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_comma_brace() {
        assert_eq!(expand_braces("{a,b,c}"), vec!["a", "b", "c"]);
    }

    #[test]
    fn test_nested_comma() {
        assert_eq!(expand_braces("x{a,b}y"), vec!["xay", "xby"]);
    }

    #[test]
    fn test_range_numeric() {
        assert_eq!(expand_braces("{1..3}"), vec!["1", "2", "3"]);
    }

    #[test]
    fn test_range_numeric_step_and_padding() {
        assert_eq!(expand_braces("{1..5..2}"), vec!["1", "3", "5"]);
        assert_eq!(expand_braces("{1..6..4}"), vec!["1", "5"]);
        assert_eq!(expand_braces("{5..1..2}"), vec!["5", "3", "1"]);
        assert_eq!(expand_braces("{01..03}"), vec!["01", "02", "03"]);
        assert_eq!(expand_braces("{-03..01..2}"), vec!["-03", "-01", "001"]);
        assert_eq!(
            expand_braces("{-003..001..2}"),
            vec!["-003", "-001", "0001"]
        );
    }

    #[test]
    fn test_range_alpha() {
        assert_eq!(expand_braces("{a..c}"), vec!["a", "b", "c"]);
    }

    #[test]
    fn test_range_alpha_step() {
        assert_eq!(expand_braces("{a..e..2}"), vec!["a", "c", "e"]);
        assert_eq!(expand_braces("{e..a..2}"), vec!["e", "c", "a"]);
    }

    #[test]
    fn test_nested_escaped_brace_preserves_literal_suffix() {
        assert_eq!(
            expand_braces(r"{x,y,\{a,b,c}}"),
            vec![r"x}", r"y}", r"\{a}", r"b}", r"c}"],
        );
    }

    #[test]
    fn test_no_brace() {
        assert_eq!(expand_braces("hello"), vec!["hello"]);
    }

    #[test]
    fn test_escaped_commas_do_not_split_brace_items() {
        assert_eq!(expand_braces(r"a{b\,c,d}"), vec![r"ab\,c", "ad"]);
        assert_eq!(expand_braces(r"{x\,y,z}"), vec![r"x\,y", "z"]);
    }

    #[test]
    fn test_escaped_braces_do_not_start_or_end_nested_groups() {
        assert_eq!(expand_braces(r"{a\{b,c}"), vec![r"a\{b", "c"]);
        assert_eq!(expand_braces(r"{a\}b,c}"), vec![r"a\}b", "c"]);
    }

    #[test]
    fn test_adjacent_brace_groups() {
        assert_eq!(expand_braces("{a,b}{1..2}"), vec!["a1", "a2", "b1", "b2"]);
    }

    #[test]
    fn test_escaped_brace_group_adjacent_to_real_group() {
        // Escaped braces stay literal (backslashes stripped later by quote
        // removal in command_prepare) while unescaped groups still expand.
        assert_eq!(expand_braces(r"\{a,b}{1,2}"), vec![r"\{a,b}1", r"\{a,b}2"]);
        assert_eq!(
            expand_braces(r"a\{b,c}d{e,f}g"),
            vec![r"a\{b,c}deg", r"a\{b,c}dfg"]
        );
    }

    #[test]
    fn test_invalid_nested_sequences_expand_only_nested_commas() {
        assert_eq!(
            expand_braces("{{1,2,3}..4}"),
            vec!["1..4", "2..4", "3..4"],
        );
        assert_eq!(
            expand_braces("{6..{7,8,9}}"),
            vec!["6..7", "6..8", "6..9"],
        );
        assert_eq!(
            expand_braces("{{a..c}..{1..3}}"),
            vec![
                "{a..1}", "{a..2}", "{a..3}", "{b..1}", "{b..2}", "{b..3}",
                "{c..1}", "{c..2}", "{c..3}",
            ],
        );
    }

    #[test]
    fn test_debug_adjacent_braces() {
        let result = expand_braces("{a,b}{1,2}");
        println!("Debug: expand_braces = {:?}", result);
        assert_eq!(result, vec!["a1", "a2", "b1", "b2"]);
    }
}
