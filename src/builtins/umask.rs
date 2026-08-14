//! `umask` builtin.
//!
//! GNU Bash source ownership:
//! - builtins/umask.def (`umask_builtin`)

use std::collections::HashMap;
use std::io::{self, Write};

const EXECUTION_SUCCESS: i32 = 0;
const EXECUTION_FAILURE: i32 = 1;

pub fn execute(args: &[String], env_vars: &mut HashMap<String, String>) -> io::Result<i32> {
    let mut stdout = io::stdout().lock();
    let mut stderr = io::stderr().lock();
    execute_with_io(args, env_vars, &mut stdout, &mut stderr)
}

pub(crate) fn execute_with_io<W, E>(
    args: &[String],
    env_vars: &mut HashMap<String, String>,
    stdout: &mut W,
    stderr: &mut E,
) -> io::Result<i32>
where
    W: Write,
    E: Write,
{
    // TODO(builtins/umask.def): GNU Bash reads and mutates the process umask.
    // This internal shell value preserves shell semantics without changing the
    // host process mask.
    let mut symbolic = false;
    let mut reusable = false;
    let mut mode = None;

    for arg in args {
        match arg.as_str() {
            value if value.starts_with('-') && value != "-" => {
                for option in value[1..].chars() {
                    match option {
                        'S' => symbolic = true,
                        'p' => reusable = true,
                        _ => {
                            writeln!(stderr, "rubash: umask: {value}: invalid option")?;
                            return Ok(EXECUTION_FAILURE);
                        }
                    }
                }
            }
            value if value.starts_with('-') => {
                writeln!(stderr, "rubash: umask: {value}: invalid option")?;
                return Ok(EXECUTION_FAILURE);
            }
            value => mode = Some(value),
        }
    }

    if let Some(mode) = mode {
        let mask = if mode.starts_with(|ch: char| ch.is_ascii_digit()) {
            match parse_mask(mode) {
                Some(mask) => mask,
                None => {
                    writeln!(stderr, "rubash: umask: {mode}: octal number out of range")?;
                    return Ok(EXECUTION_FAILURE);
                }
            }
        } else {
            match parse_symbolic_mask(mode, current_mask(env_vars)) {
                Ok(mask) => mask,
                Err(SymbolicModeError::Operator(ch)) => {
                    writeln!(
                        stderr,
                        "rubash: umask: `{ch}': invalid symbolic mode operator"
                    )?;
                    return Ok(EXECUTION_FAILURE);
                }
                Err(SymbolicModeError::Character(ch)) => {
                    writeln!(
                        stderr,
                        "rubash: umask: `{ch}': invalid symbolic mode character"
                    )?;
                    return Ok(EXECUTION_FAILURE);
                }
            }
        };
        env_vars.insert("__RUBASH_UMASK".to_string(), format!("{mask:04o}"));
        if symbolic {
            if reusable {
                writeln!(stdout, "umask -S {}", symbolic_mask(mask))?;
            } else {
                writeln!(stdout, "{}", symbolic_mask(mask))?;
            }
        }
        return Ok(EXECUTION_SUCCESS);
    }

    let mask = current_mask(env_vars);
    if reusable {
        if symbolic {
            writeln!(stdout, "umask -S {}", symbolic_mask(mask))?;
        } else {
            writeln!(stdout, "umask {mask:04o}")?;
        }
    } else if symbolic {
        writeln!(stdout, "{}", symbolic_mask(mask))?;
    } else {
        writeln!(stdout, "{mask:04o}")?;
    }

    Ok(EXECUTION_SUCCESS)
}

fn current_mask(env_vars: &HashMap<String, String>) -> u32 {
    env_vars
        .get("__RUBASH_UMASK")
        .and_then(|value| u32::from_str_radix(value, 8).ok())
        .unwrap_or(0o022)
}

fn parse_mask(mode: &str) -> Option<u32> {
    if mode.chars().all(|ch| matches!(ch, '0'..='7')) {
        return u32::from_str_radix(mode, 8).ok();
    }

    None
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum SymbolicModeError {
    Operator(char),
    Character(char),
}

fn parse_symbolic_mask(mode: &str, current_mask: u32) -> Result<u32, SymbolicModeError> {
    let initial_allowed = (!current_mask) & 0o777;
    let mut allowed = initial_allowed;

    for clause in mode.split(',') {
        if clause.is_empty() {
            return Err(SymbolicModeError::Operator('\0'));
        }
        allowed = apply_symbolic_clause(allowed, initial_allowed, clause)?;
    }

    Ok((!allowed) & 0o777)
}

fn apply_symbolic_clause(
    mut allowed: u32,
    initial_allowed: u32,
    clause: &str,
) -> Result<u32, SymbolicModeError> {
    let chars: Vec<char> = clause.chars().collect();
    let mut index = 0;
    let mut who = 0;

    while let Some(ch) = chars.get(index) {
        let bits = match ch {
            'u' => 0o700,
            'g' => 0o070,
            'o' => 0o007,
            'a' => 0o777,
            _ => break,
        };
        who |= bits;
        index += 1;
    }

    if who == 0 {
        who = 0o777;
    }

    let mut has_operator = false;
    while index < chars.len() {
        let op = chars[index];
        if !matches!(op, '+' | '-' | '=') {
            return Err(SymbolicModeError::Operator(op));
        }
        has_operator = true;
        index += 1;

        let start = index;
        while index < chars.len() && !matches!(chars[index], '+' | '-' | '=') {
            index += 1;
        }

        let perms = symbolic_permission_bits(&chars[start..index], initial_allowed, who)?;
        match op {
            '+' => allowed |= perms,
            '-' => allowed &= !perms,
            '=' => allowed = (allowed & !who) | perms,
            _ => unreachable!("validated symbolic umask operator"),
        }
    }

    if !has_operator {
        return Err(SymbolicModeError::Operator('\0'));
    }

    Ok(allowed & 0o777)
}

fn symbolic_permission_bits(
    perms: &[char],
    allowed: u32,
    who: u32,
) -> Result<u32, SymbolicModeError> {
    let mut bits = 0;
    for ch in perms {
        match ch {
            'r' => bits |= expand_permission_to_who(0o444, who),
            'w' => bits |= expand_permission_to_who(0o222, who),
            'x' => bits |= expand_permission_to_who(0o111, who),
            'u' => bits |= copy_permission_class(allowed, 0o700, who),
            'g' => bits |= copy_permission_class(allowed, 0o070, who),
            'o' => bits |= copy_permission_class(allowed, 0o007, who),
            _ => return Err(SymbolicModeError::Character(*ch)),
        }
    }
    Ok(bits)
}

fn copy_permission_class(allowed: u32, source_class: u32, who: u32) -> u32 {
    let shift = match source_class {
        0o700 => 6,
        0o070 => 3,
        0o007 => 0,
        _ => return 0,
    };
    let source = (allowed >> shift) & 0o7;
    let mut permissions = 0;
    if source & 0o4 != 0 {
        permissions |= expand_permission_to_who(0o444, who);
    }
    if source & 0o2 != 0 {
        permissions |= expand_permission_to_who(0o222, who);
    }
    if source & 0o1 != 0 {
        permissions |= expand_permission_to_who(0o111, who);
    }
    permissions
}

fn expand_permission_to_who(permission: u32, who: u32) -> u32 {
    permission & who
}

fn symbolic_mask(mask: u32) -> String {
    let allowed = (!mask) & 0o777;
    format!(
        "u={},g={},o={}",
        class_permissions((allowed & 0o700) >> 6),
        class_permissions((allowed & 0o070) >> 3),
        class_permissions(allowed & 0o007)
    )
}

fn class_permissions(bits: u32) -> String {
    let mut permissions = String::new();
    if bits & 0o4 != 0 {
        permissions.push('r');
    }
    if bits & 0o2 != 0 {
        permissions.push('w');
    }
    if bits & 0o1 != 0 {
        permissions.push('x');
    }
    permissions
}

#[cfg(test)]
mod tests {
    use super::*;

    fn run(mode: &str, initial: &str) -> (i32, String, String) {
        let mut env = HashMap::from([(String::from("__RUBASH_UMASK"), initial.to_string())]);
        let mut stdout = Vec::new();
        let mut stderr = Vec::new();
        let status = execute_with_io(
            &vec![mode.to_string(), "-S".to_string()],
            &mut env,
            &mut stdout,
            &mut stderr,
        )
        .unwrap();
        (
            status,
            String::from_utf8(stdout).unwrap(),
            String::from_utf8(stderr).unwrap(),
        )
    }

    #[test]
    fn symbolic_mode_copies_user_permissions() {
        let (status, stdout, stderr) = run("o=u", "022");
        assert_eq!(status, EXECUTION_SUCCESS);
        assert_eq!(stdout, "u=rwx,g=rx,o=rwx\n");
        assert!(stderr.is_empty());
    }

    #[test]
    fn symbolic_mode_copies_group_permissions() {
        let (status, stdout, stderr) = run("o=g", "002");
        assert_eq!(status, EXECUTION_SUCCESS);
        assert_eq!(stdout, "u=rwx,g=rwx,o=rwx\n");
        assert!(stderr.is_empty());
    }

    #[test]
    fn symbolic_mode_rejects_chmod_only_permissions() {
        for mode in ["a+X", "u+s", "u+t"] {
            let (status, stdout, stderr) = run(mode, "022");
            assert_eq!(status, EXECUTION_FAILURE);
            assert!(stdout.is_empty());
            assert!(stderr.contains("invalid symbolic mode character"));
        }
    }

    #[test]
    fn symbolic_mode_requires_an_operator() {
        for mode in ["u", "g", "a", "u,g=o"] {
            let (status, stdout, stderr) = run(mode, "022");
            assert_eq!(status, EXECUTION_FAILURE);
            assert!(stdout.is_empty());
            assert!(stderr.contains("invalid symbolic mode operator"));
        }
    }

    #[test]
    fn symbolic_copy_uses_permissions_from_before_the_clause_list() {
        let (status, stdout, stderr) = run("u=,o=u", "022");
        assert_eq!(status, EXECUTION_SUCCESS);
        assert_eq!(stdout, "u=,g=rx,o=rwx\n");
        assert!(stderr.is_empty());
    }

    #[test]
    fn invalid_modes_keep_bash_error_categories() {
        let mut env = HashMap::new();
        for (mode, expected) in [
            ("09", "rubash: umask: 09: octal number out of range\n"),
            (
                "g=p",
                "rubash: umask: `p': invalid symbolic mode character\n",
            ),
            (
                "u:rwx",
                "rubash: umask: `:': invalid symbolic mode operator\n",
            ),
        ] {
            let mut stdout = Vec::new();
            let mut stderr = Vec::new();
            let status =
                execute_with_io(&[mode.to_string()], &mut env, &mut stdout, &mut stderr).unwrap();
            assert_eq!(status, EXECUTION_FAILURE);
            assert_eq!(String::from_utf8(stderr).unwrap(), expected);
        }
    }
}
