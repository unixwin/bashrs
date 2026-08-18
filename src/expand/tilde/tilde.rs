//! tilde module.
//!
//! GNU Bash source ownership:
// - lib/tilde/tilde.c
// - lib/tilde/tilde.h

use std::collections::HashMap;

pub const QUOTED_ASSIGNMENT_VALUE: char = '\x1c';

pub fn home_value(env_vars: &HashMap<String, String>) -> String {
    // Bash's tilde expansion follows HOME when it is set. USERPROFILE is
    // only a Windows fallback for shells that have no HOME value.
    let names = ["HOME", "USERPROFILE"];

    names
        .into_iter()
        .find_map(|name| {
            env_vars
                .get(name)
                .filter(|value| !value.is_empty())
                .cloned()
                .or_else(|| std::env::var(name).ok().filter(|value| !value.is_empty()))
        })
        .unwrap_or_default()
}

pub fn expand_word_prefix(word: &str, env_vars: &HashMap<String, String>) -> Option<String> {
    if let Some(rest) = word.strip_prefix("~/") {
        return Some(format!("{}/{}", home_value(env_vars), rest));
    }

    match word {
        "~" => Some(home_value(env_vars)),
        "~+" => env_vars.get("PWD").cloned(),
        "~-" => env_vars.get("OLDPWD").cloned(),
        _ => None,
    }
}

pub fn expand_assignment_value(value: &str, env_vars: &HashMap<String, String>) -> String {
    let Some(value) = value.strip_prefix(QUOTED_ASSIGNMENT_VALUE) else {
        if !assignment_value_needs_tilde_expansion(value, true) {
            return value.to_string();
        }
        return expand_assignment_tilde_value(value, &home_value(env_vars), true);
    };

    value.to_string()
}

pub fn strip_assignment_quote_marker(value: &str) -> &str {
    value.strip_prefix(QUOTED_ASSIGNMENT_VALUE).unwrap_or(value)
}

pub fn assignment_value_needs_tilde_expansion(value: &str, expand_after_colon: bool) -> bool {
    let value = strip_assignment_quote_marker(value);
    let bytes = value.as_bytes();
    if assignment_tilde_segment_starts_at(bytes, 0) {
        return true;
    }
    if !expand_after_colon {
        return false;
    }

    bytes
        .iter()
        .enumerate()
        .any(|(index, byte)| *byte == b':' && assignment_tilde_segment_starts_at(bytes, index + 1))
}

fn assignment_tilde_segment_starts_at(bytes: &[u8], start: usize) -> bool {
    bytes.get(start) == Some(&b'~')
        && matches!(bytes.get(start + 1), None | Some(b'/') | Some(b':'))
}

pub fn expand_assignment_tilde_value(value: &str, home: &str, expand_after_colon: bool) -> String {
    if home.is_empty() {
        return value.to_string();
    }

    if !expand_after_colon {
        return expand_tilde_segment(value, home);
    }

    let mut output = String::new();
    let mut start = 0;
    for (index, ch) in value.char_indices() {
        if index == 0 || ch != ':' {
            continue;
        }
        output.push_str(&expand_tilde_segment(&value[start..index], home));
        output.push(':');
        start = index + ch.len_utf8();
    }
    output.push_str(&expand_tilde_segment(&value[start..], home));
    output
}

fn expand_tilde_segment(segment: &str, home: &str) -> String {
    let Some(rest) = segment.strip_prefix('~') else {
        return segment.to_string();
    };

    if rest.is_empty() {
        return home.to_string();
    }

    if rest.starts_with('/') {
        return format!("{home}{rest}");
    }

    segment.to_string()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[cfg(windows)]
    #[test]
    fn windows_home_value_prefers_home_before_userprofile() {
        let env_vars = HashMap::from([
            ("HOME".to_string(), "C:/from-home".to_string()),
            ("USERPROFILE".to_string(), "C:/from-userprofile".to_string()),
        ]);

        assert_eq!(home_value(&env_vars), "C:/from-home");
    }

    #[test]
    fn assignment_tilde_candidate_detects_only_expandable_segments() {
        assert!(!assignment_value_needs_tilde_expansion("plain", true));
        assert!(!assignment_value_needs_tilde_expansion("user~name", true));
        assert!(!assignment_value_needs_tilde_expansion("~user/bin", true));
        assert!(assignment_value_needs_tilde_expansion("~", true));
        assert!(assignment_value_needs_tilde_expansion("~/bin", true));
        assert!(assignment_value_needs_tilde_expansion("bin:~/tools", true));
        assert!(!assignment_value_needs_tilde_expansion(
            "bin:~/tools",
            false
        ));
    }
}
