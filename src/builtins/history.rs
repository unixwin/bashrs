//! history module.
//!
//! GNU Bash source ownership:
// - builtins/history.def

use std::io::{self, Write};

const EXECUTION_SUCCESS: i32 = 0;
const EX_USAGE: i32 = 2;

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
    let mut accepts_history_file = false;
    while let Some(arg) = args.get(index) {
        if arg == "--" {
            index += 1;
            break;
        }
        if !arg.starts_with('-') || arg == "-" {
            break;
        }

        for option in arg[1..].chars() {
            match option {
                'a' | 'n' | 'r' | 'w' => {
                    accepts_history_file = true;
                }
                'c' => {}
                'd' => {
                    index += 1;
                    if args.get(index).is_none() {
                        return Ok(EXECUTION_SUCCESS);
                    }
                    break;
                }
                'p' | 's' => {
                    return Ok(EXECUTION_SUCCESS);
                }
                other => {
                    writeln!(
                        stderr,
                        "{diagnostic_prefix}history: -{other}: invalid option"
                    )?;
                    write_usage(stderr)?;
                    return Ok(EX_USAGE);
                }
            }
        }
        index += 1;
    }

    if accepts_history_file {
        return Ok(EXECUTION_SUCCESS);
    }

    let limit = args.get(index).and_then(|arg| arg.parse::<usize>().ok());
    let start = limit.map(|n| entries.len().saturating_sub(n)).unwrap_or(0);
    for (number, entry) in entries.iter().enumerate().skip(start) {
        writeln!(stdout, "{:>5}  {}", number + 1, entry)?;
    }

    if let Some(arg) = args.get(index) {
        if !arg.chars().all(|ch| ch.is_ascii_digit()) {
            writeln!(
                stderr,
                "{diagnostic_prefix}history: {arg}: numeric argument required"
            )?;
            write_usage(stderr)?;
            return Ok(EX_USAGE);
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
        "history: usage: history [-c] [-d offset] [n] or history -anrw [filename] or history -ps arg [arg...]"
    )
}
