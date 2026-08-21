//! fc module.
//!
//! GNU Bash source ownership:
// - builtins/fc.def

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
    while let Some(arg) = args.get(index) {
        if arg == "--" {
            break;
        }
        if !arg.starts_with('-') || arg == "-" {
            break;
        }

        for (offset, option) in arg[1..].char_indices() {
            match option {
                'l' | 'n' | 'r' | 's' => {}
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

    let first = args.get(index).and_then(|arg| arg.parse::<usize>().ok());
    let start = first.unwrap_or(1).saturating_sub(1);
    for (number, entry) in entries.iter().enumerate().skip(start) {
        writeln!(stdout, "{:>5}  {}", number + 1, entry)?;
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
