//! bind module.
//!
// GNU Bash source ownership:
// - builtins/bind.def

use std::io::{self, Write};

const EXECUTION_SUCCESS: i32 = 0;
const EX_USAGE: i32 = 2;

const DEFAULT_BINDINGS: &[&str] = &[
    r#""\e[1~": beginning-of-line"#,
    r#""\e[4~": end-of-line"#,
    r#""\e[5~": beginning-of-history"#,
    r#""\e[6~": end-of-history"#,
    r#""\e[A": previous-history"#,
    r#""\e[B": next-history"#,
    r#""\e[C": forward-char"#,
    r#""\e[D": backward-char"#,
    r#""\e[3~": delete-char"#,
    r#""\e": emacs-editing-mode"#,
];

pub fn execute_with_io<E>(
    args: &[String],
    diagnostic_prefix: &str,
    stderr: &mut E,
) -> io::Result<i32>
where
    E: Write,
{
    if args.iter().any(|a| a.starts_with('-') && a.contains('p')) {
        for binding in DEFAULT_BINDINGS {
            writeln!(stderr, "{}", binding)?;
        }
        return Ok(EXECUTION_SUCCESS);
    }

    writeln!(
        stderr,
        "{diagnostic_prefix}bind: warning: line editing not enabled"
    )?;

    let mut index = 0;
    while let Some(arg) = args.get(index) {
        if arg == "--" {
            break;
        }
        if !arg.starts_with('-') || arg == "-" {
            break;
        }

        for (offset, option) in arg[1..].char_indices() {
            match option {
                'l' | 'p' | 's' | 'v' | 'P' | 'S' | 'V' | 'X' => {}
                'm' | 'f' | 'q' | 'u' | 'r' | 'x' => {
                    let value_start = 1 + offset + option.len_utf8();
                    if value_start < arg.len() {
                        break;
                    }
                    index += 1;
                    if args.get(index).is_none() {
                        writeln!(
                            stderr,
                            "{diagnostic_prefix}bind: -{option}: option requires an argument"
                        )?;
                        write_usage(stderr)?;
                        return Ok(EX_USAGE);
                    }
                }
                other => {
                    writeln!(stderr, "{diagnostic_prefix}bind: -{other}: invalid option")?;
                    write_usage(stderr)?;
                    return Ok(EX_USAGE);
                }
            }
        }
        index += 1;
    }

    Ok(EXECUTION_SUCCESS)
}

fn write_usage<E>(stderr: &mut E) -> io::Result<()>
where
    E: Write,
{
    writeln!(
        stderr,
        "bind: usage: bind [-lpsvPSVX] [-m keymap] [-f filename] [-q name] [-u name] [-r keyseq] [-x keyseq:shell-command] [keyseq:readline-function or readline-command]"
    )
}
