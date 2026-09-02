//! fc module.
//!
//! GNU Bash source ownership:
//! - builtins/fc.def

use std::io::{self, Write};

const EXECUTION_SUCCESS: i32 = 0;
const EX_USAGE: i32 = 2;

/// Returns true if arg looks like a history number (possibly negative),
/// matching bash's fc_number() so that e.g. "-1" is not misread as an option.
fn is_number_arg(arg: &str) -> bool {
    let s = if arg.starts_with('-') && arg.len() > 1 { &arg[1..] } else { arg };
    !s.is_empty() && s.parse::<isize>().is_ok()
}

pub fn execute_with_io<E>(
    args: &[String],
    diagnostic_prefix: &str,
    stderr: &mut E,
) -> io::Result<i32>
where
    E: Write,
{
    execute_with_history(args, diagnostic_prefix, &[], &mut io::sink(), stderr)
}

pub fn execute_with_history<E, O>(
    args: &[String],
    diagnostic_prefix: &str,
    entries: &[String],
    stdout: &mut O,
    stderr: &mut E,
) -> io::Result<i32>
where
    E: Write,
    O: Write,
{
    let mut index = 0;
    let mut numbering = true;
    let mut reverse = false;
    let mut _listing = false;
    let mut _execute = false;

    while let Some(arg) = args.get(index) {
        if arg == "--" {
            break;
        }
        if arg == "--help" || arg == "-h" {
            write_help(stdout)?;
            return Ok(EXECUTION_SUCCESS);
        }
        if !arg.starts_with('-') || arg == "-" || is_number_arg(arg) {
            break;
        }

        for (offset, option) in arg[1..].char_indices() {
            match option {
                'l' => {
                    _listing = true;
                }
                'n' => {
                    numbering = false;
                }
                'r' => {
                    reverse = true;
                }
                's' => {
                    _execute = true;
                }
                'e' => {
                    let value_start = 1 + offset + option.len_utf8();
                    if value_start < arg.len() {
                        break;
                    }
                    index += 1;
                    if args.get(index).is_none() {
                        writeln!(
                            stderr,
                            "{diagnostic_prefix}fc: -e: option requires an argument"
                        )?;
                        write_usage(stderr)?;
                        return Ok(EX_USAGE);
                    }
                }
                other => {
                    writeln!(stderr, "{diagnostic_prefix}fc: -{other}: invalid option")?;
                    write_usage(stderr)?;
                    return Ok(EX_USAGE);
                }
            }
        }
        index += 1;
    }

    if _execute {
        return Ok(EXECUTION_SUCCESS);
    }

    let effective_len: usize = entries.len();

    if effective_len == 0 {
        return Ok(EXECUTION_SUCCESS);
    }
    let first: Option<isize> = args.get(index).and_then(|s| s.parse().ok());
    let last: Option<isize> = args.get(index + 1).and_then(|s| s.parse().ok());

    let resolve_start = |val: Option<isize>, eff_len: usize| -> usize {
        match val {
            None => 0,
            Some(0) => eff_len,
            Some(n) if n < 0 => (eff_len as isize - n.abs() as isize).max(0) as usize,
            Some(n) => (n - 1).max(0) as usize,
        }
    };

    let resolve_end = |val: Option<isize>, eff_len: usize| -> usize {
        match val {
            None => eff_len,
            Some(0) => eff_len,
            Some(n) if n < 0 => (eff_len as isize - n.abs() as isize).max(0) as usize,
            Some(n) => n as usize,
        }
    };

    let start_idx = resolve_start(first, effective_len).min(effective_len);
    let end_idx = resolve_end(last, effective_len).min(effective_len);

    if start_idx >= end_idx {
        return Ok(EXECUTION_SUCCESS);
    }

    let slice = &entries[start_idx..end_idx];

    if reverse {
        for (i, entry) in slice.iter().rev().enumerate() {
            let original_i = slice.len() - 1 - i;
            let history_num = start_idx + original_i + 1;
            if numbering {
                writeln!(stdout, "{:>5}  {}", history_num, entry)?;
            } else {
                writeln!(stdout, "{}", entry)?;
            }
        }
    } else {
        for (i, entry) in slice.iter().enumerate() {
            let history_num = start_idx + i + 1;
            if numbering {
                writeln!(stdout, "{:>5}  {}", history_num, entry)?;
            } else {
                writeln!(stdout, "{}", entry)?;
            }
        }
    }

    Ok(EXECUTION_SUCCESS)
}

fn write_usage<E>(stderr: &mut E) -> io::Result<()>
where
    E: Write,
{
    writeln!(
        stderr,
        "fc: usage: fc [-e ename] [-lnr] [first] [last] or fc -s [pat=rep] [command]"
    )
}

fn write_help<O>(stdout: &mut O) -> io::Result<()>
where
    O: Write,
{
    writeln!(stdout, "fc: display or execute commands from the history list")?;
    writeln!(stdout, "")?;
    writeln!(stdout, "Usage: fc [-e ename] [-lnr] [first] [last]")?;
    writeln!(stdout, "       fc -s [pat=rep ...] [command]")?;
    writeln!(stdout, "")?;
    writeln!(stdout, "Display or execute commands from the history list.")?;
    writeln!(stdout, "")?;
    writeln!(stdout, "Options:")?;
    writeln!(stdout, "  -e ename    Select which editor to use.")?;
    writeln!(stdout, "  -l          List lines instead of editing.")?;
    writeln!(stdout, "  -n          Omit line numbers when listing.")?;
    writeln!(stdout, "  -r          Reverse the order of the lines.")?;
    writeln!(stdout, "  -s          Re-execute command after substitution.")?;
    writeln!(stdout, "")?;
    writeln!(stdout, "FIRST and LAST can be numbers or strings.")?;
    writeln!(stdout, "Negative numbers count back from the most recent command.")?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn run_fc(args: &[&str], entries: &[&str]) -> (String, String, i32) {
        let args: Vec<String> = args.iter().map(|s| s.to_string()).collect();
        let entries: Vec<String> = entries.iter().map(|s| s.to_string()).collect();
        let mut stdout = Vec::new();
        let mut stderr = Vec::new();
        let status =
            execute_with_history(&args, "fc: ", &entries, &mut stdout, &mut stderr).unwrap();
        (
            String::from_utf8(stdout).unwrap(),
            String::from_utf8(stderr).unwrap(),
            status,
        )
    }

    fn expect_lines(out: &str, lines: &[&str]) {
        let actual: Vec<&str> = out.lines().collect();
        assert_eq!(actual, lines);
    }

    // --- -n (no numbering) ---

    #[test]
    fn test_fc_no_numbering() {
        let (out, _, status) = run_fc(&["-ln"], &["a", "b", "c", "fc"]);
        assert_eq!(status, EXECUTION_SUCCESS);
        expect_lines(&out, &["a", "b", "c"]);
    }

    // --- -r (reverse) ---

    #[test]
    fn test_fc_reverse_all() {
        let (out, _, status) = run_fc(&["-lr"], &["a", "b", "c", "d", "fc"]);
        assert_eq!(status, EXECUTION_SUCCESS);
        expect_lines(&out, &["    4  d", "    3  c", "    2  b", "    1  a"]);
    }

    #[test]
    fn test_fc_reverse_with_range() {
        let (out, _, status) = run_fc(&["-lr", "2", "4"], &["a", "b", "c", "d", "fc"]);
        assert_eq!(status, EXECUTION_SUCCESS);
        expect_lines(&out, &["    4  d", "    3  c", "    2  b"]);
    }

    #[test]
    fn test_fc_reverse_no_numbers() {
        let (out, _, status) = run_fc(&["-lnr"], &["a", "b", "c", "fc"]);
        assert_eq!(status, EXECUTION_SUCCESS);
        expect_lines(&out, &["c", "b", "a"]);
    }

    // --- negative first ---

    #[test]
    fn test_fc_negative_first_minus_one() {
        let (out, _, status) = run_fc(&["-ln", "-1"], &["a", "b", "c", "git status", "fc"]);
        assert_eq!(status, EXECUTION_SUCCESS);
        expect_lines(&out, &["git status"]);
    }

    #[test]
    fn test_fc_negative_first_minus_two() {
        let (out, _, status) = run_fc(&["-ln", "-2"], &["a", "b", "c", "git status", "fc"]);
        assert_eq!(status, EXECUTION_SUCCESS);
        expect_lines(&out, &["c", "git status"]);
    }

    #[test]
    fn test_fc_negative_first_clamped_to_zero() {
        let (out, _, status) = run_fc(&["-ln", "-10"], &["a", "b", "c", "fc"]);
        assert_eq!(status, EXECUTION_SUCCESS);
        expect_lines(&out, &["a", "b", "c"]);
    }

    // --- last argument ---

    #[test]
    fn test_fc_positive_range() {
        let (out, _, status) = run_fc(&["-l", "2", "4"], &["a", "b", "c", "d", "fc"]);
        assert_eq!(status, EXECUTION_SUCCESS);
        expect_lines(&out, &["    2  b", "    3  c", "    4  d"]);
    }

    #[test]
    fn test_fc_negative_first_and_last() {
        let (out, _, status) = run_fc(&["-ln", "-3", "-1"], &["a", "b", "c", "d", "fc"]);
        assert_eq!(status, EXECUTION_SUCCESS);
        expect_lines(&out, &["b", "c"]);
    }

    #[test]
    fn test_fc_last_clamped() {
        let (out, _, status) = run_fc(&["-l", "1", "100"], &["a", "b", "c", "d", "fc"]);
        assert_eq!(status, EXECUTION_SUCCESS);
        expect_lines(&out, &["    1  a", "    2  b", "    3  c", "    4  d"]);
    }

    #[test]
    fn test_fc_empty_range_no_output() {
        let (out, _, status) = run_fc(&["-l", "5", "3"], &["a", "b", "c", "d", "fc"]);
        assert_eq!(status, EXECUTION_SUCCESS);
        assert_eq!(out, "");
    }

    // --- defaults ---

    #[test]
    fn test_fc_default_all_entries() {
        let (out, _, status) = run_fc(&["-l"], &["a", "b", "c", "fc"]);
        assert_eq!(status, EXECUTION_SUCCESS);
        expect_lines(&out, &["    1  a", "    2  b", "    3  c"]);
    }

    #[test]
    fn test_fc_empty_entries() {
        let (out, _, status) = run_fc(&["-l"], &[]);
        assert_eq!(status, EXECUTION_SUCCESS);
        assert_eq!(out, "");
    }

    // --- --help ---

    #[test]
    fn test_fc_help_flag() {
        let (out, _, status) = run_fc(&["--help"], &[]);
        assert_eq!(status, EXECUTION_SUCCESS);
        assert!(out.contains("Usage: fc"));
        assert!(out.contains("-e ename"));
        assert!(out.contains("-n"));
        assert!(out.contains("-r"));
    }
}
