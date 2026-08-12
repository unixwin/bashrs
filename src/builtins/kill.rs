//! kill module.
//!
//! GNU Bash source ownership:
// - builtins/kill.def

use std::io::{self, Write};

const SIGNALS: &[(i32, &str, &str)] = &[
    (1, "1", "HUP"),
    (2, "2", "INT"),
    (3, "3", "QUIT"),
    (4, "4", "ILL"),
    (5, "5", "TRAP"),
    (6, "6", "ABRT"),
    (7, "7", "EMT"),
    (8, "8", "FPE"),
    (9, "9", "KILL"),
    (10, "10", "BUS"),
    (11, "11", "SEGV"),
    (12, "12", "SYS"),
    (13, "13", "PIPE"),
    (14, "14", "ALRM"),
    (15, "15", "TERM"),
    (16, "16", "URG"),
    (17, "17", "STOP"),
    (18, "18", "TSTP"),
    (19, "19", "CONT"),
    (20, "20", "CHLD"),
    (21, "21", "TTIN"),
    (22, "22", "TTOU"),
    (23, "23", "IO"),
    (24, "24", "XCPU"),
    (25, "25", "XFSZ"),
    (26, "26", "VTALRM"),
    (27, "27", "PROF"),
    (28, "28", "WINCH"),
    (29, "29", "PWR"),
    (30, "30", "USR1"),
    (31, "31", "USR2"),
    (32, "32", "RTMIN"),
    (33, "33", "RTMIN+1"),
    (34, "34", "RTMIN+2"),
    (35, "35", "RTMIN+3"),
    (36, "36", "RTMIN+4"),
    (37, "37", "RTMIN+5"),
    (38, "38", "RTMIN+6"),
    (39, "39", "RTMIN+7"),
    (40, "40", "RTMIN+8"),
    (41, "41", "RTMIN+9"),
    (42, "42", "RTMIN+10"),
    (43, "43", "RTMIN+11"),
    (44, "44", "RTMIN+12"),
    (45, "45", "RTMIN+13"),
    (46, "46", "RTMIN+14"),
    (47, "47", "RTMIN+15"),
    (48, "48", "RTMIN+16"),
    (49, "49", "RTMAX-15"),
    (50, "50", "RTMAX-14"),
    (51, "51", "RTMAX-13"),
    (52, "52", "RTMAX-12"),
    (53, "53", "RTMAX-11"),
    (54, "54", "RTMAX-10"),
    (55, "55", "RTMAX-9"),
    (56, "56", "RTMAX-8"),
    (57, "57", "RTMAX-7"),
    (58, "58", "RTMAX-6"),
    (59, "59", "RTMAX-5"),
    (60, "60", "RTMAX-4"),
    (61, "61", "RTMAX-3"),
    (62, "62", "RTMAX-2"),
    (63, "63", "RTMAX-1"),
    (64, "64", "RTMAX"),
];

pub fn execute(args: &[String]) -> io::Result<i32> {
    let mut stdout = io::stdout().lock();
    let mut stderr = io::stderr().lock();
    execute_with_io(args, &mut stdout, &mut stderr)
}

pub fn execute_with_io<W, E>(args: &[String], stdout: &mut W, stderr: &mut E) -> io::Result<i32>
where
    W: Write,
    E: Write,
{
    let Some(first) = args.first().map(String::as_str) else {
        write_kill_usage(stderr)?;
        return Ok(2);
    };

    if matches!(first, "-l" | "-L") {
        if args.len() == 1 || args.get(1).is_some_and(|value| value == "-1") {
            write_signal_list(stdout)?;
            return Ok(0);
        }
        let mut status = 0;
        for value in &args[1..] {
            if let Some(translation) = translate_signal(value) {
                writeln!(stdout, "{translation}")?;
            } else {
                writeln!(
                    stderr,
                    "{}kill: {value}: invalid signal specification",
                    diagnostic_prefix()
                )?;
                status = 1;
            }
        }
        return Ok(status);
    }

    let mut signal = 15;
    let mut index = 0;
    let mut operands_start = 0;
    while let Some(value) = args.get(index).map(String::as_str) {
        if value == "--" {
            operands_start = index + 1;
            break;
        }
        if value == "-s" || value == "-n" {
            let Some(sigspec) = args.get(index + 1).map(String::as_str) else {
                write_kill_usage(stderr)?;
                return Ok(2);
            };
            if translate_signal(sigspec).is_none() {
                writeln!(
                    stderr,
                    "{}kill: {sigspec}: invalid signal specification",
                    diagnostic_prefix()
                )?;
                return Ok(1);
            }
            signal = signal_number_from_spec(sigspec).unwrap_or(15);
            index += 2;
            operands_start = index;
            continue;
        }
        if value.starts_with('-') && value != "-" {
            let sigspec = value.trim_start_matches('-');
            if translate_signal(sigspec).is_none() {
                writeln!(
                    stderr,
                    "{}kill: {sigspec}: invalid signal specification",
                    diagnostic_prefix()
                )?;
                return Ok(1);
            }
            signal = signal_number_from_spec(sigspec).unwrap_or(15);
            index += 1;
            operands_start = index;
            continue;
        }
        operands_start = index;
        break;
    }

    if operands_start >= args.len() {
        write_kill_usage(stderr)?;
        return Ok(2);
    }

    let mut status = 0;
    for operand in &args[operands_start..] {
        let Some(pid) = parse_pid(operand) else {
            writeln!(
                stderr,
                "{}kill: {operand}: arguments must be process or job IDs",
                diagnostic_prefix()
            )?;
            status = 1;
            continue;
        };

        if let Err(message) = signal_process(pid, signal) {
            writeln!(stderr, "{}kill: ({pid}) - {message}", diagnostic_prefix())?;
            status = 1;
        }
    }

    Ok(status)
}

fn write_kill_usage<E>(stderr: &mut E) -> io::Result<()>
where
    E: Write,
{
    writeln!(
        stderr,
        "{}kill: usage: kill [-s sigspec | -n signum | -sigspec] pid | jobspec ... or kill -l [sigspec]",
        diagnostic_prefix()
    )
}

pub fn list_first_signal_for_sed() -> &'static str {
    "SIGHUP"
}

pub fn translate_signal(value: &str) -> Option<&'static str> {
    if value == "0" {
        return Some("EXIT");
    }

    if value == "EXIT" || value == "SIGEXIT" {
        return Some("0");
    }

    if let Ok(mut number) = value.parse::<i32>() {
        if number > 128 {
            number -= 128;
        }
        return signal_name(number);
    }

    let name = value.strip_prefix("SIG").unwrap_or(value);
    signal_number(name)
}

fn signal_number_from_spec(value: &str) -> Option<i32> {
    if value == "0" {
        return Some(0);
    }

    let name = value.strip_prefix("SIG").unwrap_or(value);
    if name == "EXIT" {
        return Some(0);
    }

    if let Ok(mut number) = value.parse::<i32>() {
        if number > 128 {
            number -= 128;
        }
        return (number == 0 || signal_name(number).is_some()).then_some(number);
    }

    signal_number(name)?.parse::<i32>().ok()
}

fn parse_pid(value: &str) -> Option<u32> {
    let pid = value.parse::<u32>().ok()?;
    (pid != 0).then_some(pid)
}

pub fn process_exists(pid: u32) -> bool {
    pid == std::process::id() || signal_process(pid, 0).is_ok_or_permission_denied()
}

fn signal_name(number: i32) -> Option<&'static str> {
    SIGNALS
        .iter()
        .find_map(|(signal_number, _, name)| (*signal_number == number).then_some(*name))
}

fn signal_number(name: &str) -> Option<&'static str> {
    SIGNALS
        .iter()
        .find_map(|(_, number, signal_name)| (*signal_name == name).then_some(*number))
}

fn write_signal_list<W>(stdout: &mut W) -> io::Result<()>
where
    W: Write,
{
    for chunk in SIGNALS.chunks(5) {
        for (index, (number, _, name)) in chunk.iter().enumerate() {
            if index > 0 {
                write!(stdout, "\t")?;
            }
            write!(stdout, "{number:>2}) SIG{name}")?;
        }
        writeln!(stdout)?;
    }
    Ok(())
}

fn diagnostic_prefix() -> String {
    if let (Ok(script), Ok(line)) = (
        std::env::var("__RUBASH_SCRIPT_NAME"),
        std::env::var("__RUBASH_CURRENT_LINE"),
    ) {
        return format!("{script}: line {line}: ");
    }

    "rubash: ".to_string()
}

#[cfg(windows)]
fn signal_process(pid: u32, signal: i32) -> Result<(), &'static str> {
    use windows_sys::Win32::Foundation::{
        CloseHandle, GetLastError, ERROR_ACCESS_DENIED, ERROR_INVALID_PARAMETER, STILL_ACTIVE,
    };
    use windows_sys::Win32::System::Threading::{
        GetExitCodeProcess, OpenProcess, TerminateProcess, PROCESS_QUERY_INFORMATION,
        PROCESS_QUERY_LIMITED_INFORMATION, PROCESS_TERMINATE,
    };

    const SYNCHRONIZE_ACCESS: u32 = 0x0010_0000;

    let access = if signal == 0 {
        PROCESS_QUERY_LIMITED_INFORMATION
    } else {
        PROCESS_QUERY_INFORMATION | PROCESS_TERMINATE | SYNCHRONIZE_ACCESS
    };

    let handle = unsafe { OpenProcess(access, 0, pid) };
    if handle.is_null() {
        let error = unsafe { GetLastError() };
        return Err(match error {
            ERROR_ACCESS_DENIED => "Permission denied",
            ERROR_INVALID_PARAMETER => "No such process",
            _ => "Cannot open process",
        });
    }

    let mut exit_code = 0;
    let process_is_active = unsafe {
        GetExitCodeProcess(handle, &mut exit_code) != 0 && exit_code == STILL_ACTIVE as u32
    };
    if !process_is_active {
        unsafe {
            CloseHandle(handle);
        }
        return Err("No such process");
    }

    if signal != 0 && unsafe { TerminateProcess(handle, 1) == 0 } {
        unsafe {
            CloseHandle(handle);
        }
        return Err("Failed to terminate process");
    }

    unsafe {
        CloseHandle(handle);
    }
    Ok(())
}

#[cfg(unix)]
fn signal_process(pid: u32, signal: i32) -> Result<(), &'static str> {
    let result = unsafe { libc::kill(pid as libc::pid_t, signal) };
    if result == 0 {
        return Ok(());
    }

    match std::io::Error::last_os_error().raw_os_error() {
        Some(libc::ESRCH) => Err("No such process"),
        Some(libc::EPERM) => Err("Permission denied"),
        _ => Err("Failed to signal process"),
    }
}

#[cfg(not(any(unix, windows)))]
fn signal_process(_pid: u32, signal: i32) -> Result<(), &'static str> {
    if signal == 0 {
        Err("No such process")
    } else {
        Err("Failed to signal process")
    }
}

trait KillResultExt {
    fn is_ok_or_permission_denied(&self) -> bool;
}

impl KillResultExt for Result<(), &'static str> {
    fn is_ok_or_permission_denied(&self) -> bool {
        self.is_ok() || matches!(self, Err(message) if *message == "Permission denied")
    }
}
