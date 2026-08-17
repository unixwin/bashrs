use super::ParsedNumber;

pub(super) fn parse_i64(value: &str) -> ParsedNumber<i64> {
    if let Some(ch) = printf_char_constant(value) {
        return ParsedNumber {
            value: ch as i64,
            invalid: None,
        };
    }
    if value.is_empty() {
        return ParsedNumber {
            value: 0,
            invalid: None,
        };
    }
    match parse_integer_literal(value) {
        Some((parsed_value, has_invalid_suffix)) => ParsedNumber {
            value: parsed_value,
            invalid: has_invalid_suffix.then(|| value.to_string()),
        },
        None => ParsedNumber {
            value: 0,
            invalid: Some(value.to_string()),
        },
    }
}

pub(super) fn parse_f64(value: &str) -> ParsedNumber<f64> {
    if let Some(ch) = printf_char_constant(value) {
        return ParsedNumber {
            value: ch as u32 as f64,
            invalid: None,
        };
    }
    if value.is_empty() {
        return ParsedNumber {
            value: 0.0,
            invalid: None,
        };
    }
    match value.parse::<f64>() {
        Ok(value) => ParsedNumber {
            value,
            invalid: None,
        },
        Err(_) => ParsedNumber {
            value: 0.0,
            invalid: Some(value.to_string()),
        },
    }
}

pub(super) fn invalid_number_error(value: &str) -> String {
    format!("rubash: printf: {value}: invalid number")
}

fn printf_char_constant(value: &str) -> Option<char> {
    let mut chars = value.chars();
    match chars.next() {
        Some('\'') | Some('"') => chars.next(),
        _ => None,
    }
}

fn parse_integer_literal(value: &str) -> Option<(i64, bool)> {
    let value = value.trim();
    let (sign, digits) = match value.as_bytes().first().copied() {
        Some(b'-') => (-1_i64, &value[1..]),
        Some(b'+') => (1_i64, &value[1..]),
        _ => (1_i64, value),
    };

    let (radix, digits) = if let Some(hex) = digits
        .strip_prefix("0x")
        .or_else(|| digits.strip_prefix("0X"))
    {
        (16, hex)
    } else if digits.len() > 1 && digits.starts_with('0') {
        (8, digits)
    } else {
        (10, digits)
    };

    // Bash's printf uses the valid numeric prefix even when the remainder is
    // invalid, while still returning an error for the argument.  For example,
    // `%d` formats `1.2` as `1` and `08` as `0`.
    let prefix_len = digits
        .char_indices()
        .take_while(|(_, ch)| ch.to_digit(radix).is_some())
        .map(|(index, ch)| index + ch.len_utf8())
        .last()
        .unwrap_or(0);
    if prefix_len == 0 {
        return None;
    }

    let parsed = i64::from_str_radix(&digits[..prefix_len], radix).ok()?;
    Some((sign * parsed, prefix_len != digits.len()))
}
