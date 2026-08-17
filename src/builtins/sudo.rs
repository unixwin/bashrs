//! `sudo` builtin option parsing.
//!
//! This is a rubash host-capability builtin: the shell parses the request, then
//! delegates elevation to the embedding host through `Executor::set_elevation_handler`.

use std::io::{self, Write};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SudoMode {
    Inline,
    NewWindow,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SudoInvocation {
    pub preserve_environment: bool,
    pub mode: SudoMode,
    pub command: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum SudoAction {
    Run(SudoInvocation),
    Complete(i32),
}

pub fn parse(args: &[String]) -> Result<SudoAction, String> {
    let mut preserve_environment = false;
    let mut mode = SudoMode::Inline;
    let mut index = 0;

    while index < args.len() {
        let arg = args[index].as_str();
        match arg {
            "--" => {
                index += 1;
                break;
            }
            "-h" | "--help" => return Ok(SudoAction::Complete(0)),
            "-E" | "--preserve-env" => {
                preserve_environment = true;
                index += 1;
            }
            "--inline" => {
                mode = SudoMode::Inline;
                index += 1;
            }
            "--new-window" | "--newWindow" => {
                mode = SudoMode::NewWindow;
                index += 1;
            }
            _ if is_clustered_short_option(arg) => {
                for option in arg[1..].chars() {
                    match option {
                        'E' => preserve_environment = true,
                        'h' => return Ok(SudoAction::Complete(0)),
                        other => return Err(format!("invalid option -- '{other}'")),
                    }
                }
                index += 1;
            }
            _ if arg.starts_with('-') && arg != "-" => {
                return Err(format!("invalid option: {arg}"));
            }
            _ => break,
        }
    }

    let command = args[index..].to_vec();
    if command.is_empty() {
        return Err("a command is required".to_string());
    }

    Ok(SudoAction::Run(SudoInvocation {
        preserve_environment,
        mode,
        command,
    }))
}

fn is_clustered_short_option(arg: &str) -> bool {
    arg.starts_with('-') && !arg.starts_with("--") && arg.len() > 2
}

pub fn print_help_with_io<W>(stdout: &mut W) -> io::Result<()>
where
    W: Write,
{
    writeln!(stdout, "sudo: sudo [-E] [--inline|--new-window] [--] command [arg ...]")?;
    writeln!(stdout, "    Run a command through the host elevation provider.")?;
    writeln!(stdout, "")?;
    writeln!(stdout, "    Options:")?;
    writeln!(stdout, "      -E, --preserve-env    request the current shell environment")?;
    writeln!(stdout, "          --inline          request inline elevated execution")?;
    writeln!(stdout, "          --new-window      request execution in a new elevated window")?;
    writeln!(stdout, "")?;
    writeln!(stdout, "    The shell parses sudo as a builtin, but elevation is supplied by the")?;
    writeln!(stdout, "    embedding host through Executor::set_elevation_handler.")
}

#[cfg(test)]
mod tests {
    use super::*;

    fn words(values: &[&str]) -> Vec<String> {
        values.iter().map(|value| value.to_string()).collect()
    }

    #[test]
    fn parses_basic_command() {
        let action = parse(&words(&["whoami"])).unwrap();
        assert_eq!(
            action,
            SudoAction::Run(SudoInvocation {
                preserve_environment: false,
                mode: SudoMode::Inline,
                command: words(&["whoami"]),
            })
        );
    }

    #[test]
    fn parses_options_before_command() {
        let action = parse(&words(&["-E", "--new-window", "--", "rubash", "-c", "echo hi"]))
            .unwrap();
        assert_eq!(
            action,
            SudoAction::Run(SudoInvocation {
                preserve_environment: true,
                mode: SudoMode::NewWindow,
                command: words(&["rubash", "-c", "echo hi"]),
            })
        );
    }

    #[test]
    fn rejects_missing_command() {
        assert_eq!(parse(&words(&["-E"])), Err("a command is required".to_string()));
    }
}
