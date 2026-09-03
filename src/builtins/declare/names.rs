/// GNU general.c:317 valid_array_reference: a nameref value (or declared
/// nameref name) may be `name[subscript]` with a valid identifier base and a
/// non-empty subscript. The full GNU version also validates quoted subscripts
/// and `[@]`/`[*]` forms; declare-time checks only need the reject side, so a
/// conservative shape test is enough here.
pub(super) fn valid_array_reference(arg: &str) -> bool {
    let Some(open) = arg.find('[') else {
        return false;
    };
    if !arg.ends_with(']') {
        return false;
    }
    let base = &arg[..open];
    let subscript = &arg[open + 1..arg.len() - 1];
    !subscript.is_empty() && valid_identifier(base)
}

/// GNU general.c:310 valid_nameref_value: a nameref value must be a valid
/// identifier or (flags != 2) a valid array reference.
pub(super) fn valid_nameref_value(value: &str) -> bool {
    !value.is_empty() && (valid_identifier(value) || valid_array_reference(value))
}

/// GNU general.c:327 check_selfref: the value references the nameref variable
/// itself, either directly (`declare -n x=x`) or through an array element of
/// the same array (`declare -n x=x[1]`).
pub(super) fn check_selfref(name: &str, value: &str) -> bool {
    if name == value {
        return true;
    }
    if valid_array_reference(value) {
        if let Some(open) = value.find('[') {
            return &value[..open] == name;
        }
    }
    false
}

pub(super) fn declare_base_name(arg: &str) -> Option<&str> {
    let name = arg.split_once('=').map(|(name, _)| name).unwrap_or(arg);
    let name = name.strip_suffix('+').unwrap_or(name);
    let name = name.split_once('[').map(|(name, _)| name).unwrap_or(name);
    valid_identifier(name).then_some(name)
}

pub(super) fn valid_declare_name(arg: &str) -> bool {
    let name = arg.split_once('=').map(|(name, _)| name).unwrap_or(arg);
    let name = name.strip_suffix('+').unwrap_or(name);
    if let Some((base, subscript)) = name.split_once('[') {
        let Some(subscript) = subscript.strip_suffix(']') else {
            return false;
        };
        return !subscript.is_empty() && valid_identifier(base);
    }
    valid_identifier(name)
}

pub(super) fn valid_identifier(name: &str) -> bool {
    let mut chars = name.chars();
    let Some(first) = chars.next() else {
        return false;
    };
    (first == '_' || first.is_ascii_alphabetic())
        && chars.all(|ch| ch == '_' || ch.is_ascii_alphanumeric())
}
