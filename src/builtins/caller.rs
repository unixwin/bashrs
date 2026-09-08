//! caller module.
//!
//! GNU Bash source ownership:
// - builtins/caller.def

use std::io::{self, Write};

const EXECUTION_SUCCESS: i32 = 0;
const EXECUTION_FAILURE: i32 = 1;
const EX_USAGE: i32 = 2;

pub fn execute_with_io<W, E>(
    args: &[String],
    funcname: &[String],
    lineno: &[String],
    source: &[String],
    diagnostic_prefix: &str,
    stdout: &mut W,
    stderr: &mut E,
) -> io::Result<i32>
where
    W: Write,
    E: Write,
{
    let args = if args.first().is_some_and(|arg| arg == "--") {
        &args[1..]
    } else {
        args
    };

    let level = match args.first() {
        Some(arg) if arg.starts_with('-') => {
            writeln!(stderr, "{diagnostic_prefix}caller: {arg}: invalid option")?;
            writeln!(stderr, "caller: usage: caller [expr]")?;
            return Ok(EX_USAGE);
        }
        Some(arg) => match arg.parse::<usize>() {
            Ok(level) => Some(level),
            Err(_) => {
                writeln!(stderr, "{diagnostic_prefix}caller: {arg}: invalid number")?;
                writeln!(stderr, "caller: usage: caller [expr]")?;
                return Ok(EX_USAGE);
            }
        },
        None => None,
    };

    match level {
        Some(level) => print_call_frame(level, funcname, lineno, source, stdout),
        None => print_current_call(funcname, lineno, source, stdout),
    }
}

fn print_current_call<W>(
    funcname: &[String],
    lineno: &[String],
    source: &[String],
    stdout: &mut W,
) -> io::Result<i32>
where
    W: Write,
{
    // GNU builtins/caller.def:89-105: the no-argument form never consults
    // FUNCNAME. It fails only when the BASH_LINENO or BASH_SOURCE array is
    // empty, then prints BASH_LINENO[0] and BASH_SOURCE[1] with a literal
    // NULL for absent elements. At the top level that renders "0 NULL"
    // (dbg-support.tests line 71); inside a function called from line N of
    // file F it renders "N F".
    let _ = funcname;
    if lineno.is_empty() || source.is_empty() {
        return Ok(EXECUTION_FAILURE);
    }
    let line = lineno.first().map(String::as_str).unwrap_or("NULL");
    let caller_source = source.get(1).map(String::as_str).unwrap_or("NULL");
    writeln!(stdout, "{line} {caller_source}")?;
    Ok(EXECUTION_SUCCESS)
}

fn print_call_frame<W>(
    level: usize,
    funcname: &[String],
    lineno: &[String],
    source: &[String],
    stdout: &mut W,
) -> io::Result<i32>
where
    W: Write,
{
    // GNU builtins/caller.def:108-121: with EXPR the frame is
    // BASH_LINENO[expr] FUNCNAME[expr+1] BASH_SOURCE[expr+1]; any absent
    // element fails the builtin with status 1 and no output (no "0" /
    // "environment" fallbacks).
    if funcname.is_empty() {
        return Ok(EXECUTION_FAILURE);
    }
    let Some(line) = lineno.get(level) else {
        return Ok(EXECUTION_FAILURE);
    };
    let Some(function) = funcname.get(level + 1) else {
        return Ok(EXECUTION_FAILURE);
    };
    let Some(caller_source) = source.get(level + 1) else {
        return Ok(EXECUTION_FAILURE);
    };
    writeln!(stdout, "{line} {function} {caller_source}")?;
    Ok(EXECUTION_SUCCESS)
}
