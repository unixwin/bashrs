use crate::executor::Executor;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ShellInvocation {
    pub posix: bool,
    pub login: bool,
    pub no_profile: bool,
    pub no_rc: bool,
    pub command: Option<String>,
    pub command_name: Option<String>,
    pub positional_params: Vec<String>,
    pub script: Option<String>,
    pub read_stdin: bool,
    pub shell_flags: Vec<(String, bool)>,
    pub shopt_flags: Vec<(String, bool)>,
}

impl ShellInvocation {
    pub fn parse(args: &[String]) -> Result<Self, String> {
        let mut expanded = Vec::with_capacity(args.len());
        for arg in args {
            if let Some(flags) = arg.strip_prefix('-') {
                let flags = flags.strip_prefix('-').unwrap_or(flags);
                if flags.len() > 1
                    && flags.contains('c')
                    && flags
                        .chars()
                        .all(|f| f == 'c' || f == 's' || cli_flag_name(f).is_some())
                {
                    for flag in flags.chars() {
                        expanded.push(if flag == 'c' {
                            "-c".to_string()
                        } else {
                            format!("-{flag}")
                        });
                    }
                    continue;
                }
            }
            expanded.push(arg.clone());
        }

        let mut out = Self {
            posix: false,
            login: false,
            no_profile: false,
            no_rc: false,
            command: None,
            command_name: None,
            positional_params: Vec::new(),
            script: None,
            read_stdin: false,
            shell_flags: Vec::new(),
            shopt_flags: Vec::new(),
        };
        let mut i = 0usize;
        while i < expanded.len() {
            let arg = expanded[i].as_str();
            match arg {
                "-c" => {
                    let command = expanded
                        .get(i + 1)
                        .ok_or_else(|| "-c: option requires an argument".to_string())?;
                    out.command = Some(command.clone());
                    if let Some(name) = expanded.get(i + 2) {
                        out.command_name = Some(name.clone());
                        out.positional_params = expanded[i + 3..].to_vec();
                    }
                    return Ok(out);
                }
                "-s" => {
                    out.read_stdin = true;
                    out.positional_params = expanded[i + 1..].to_vec();
                    return Ok(out);
                }
                "--posix" => {
                    out.posix = true;
                    i += 1;
                }
                "--login" | "-l" => {
                    out.login = true;
                    i += 1;
                }
                "--noprofile" => {
                    out.no_profile = true;
                    i += 1;
                }
                "--norc" => {
                    out.no_rc = true;
                    i += 1;
                }
                "-o" | "+o" => {
                    let name = expanded
                        .get(i + 1)
                        .ok_or_else(|| format!("{arg}: option requires an argument"))?;
                    out.shell_flags.push((name.clone(), arg == "-o"));
                    i += 2;
                }
                "-O" | "+O" => {
                    let name = expanded
                        .get(i + 1)
                        .ok_or_else(|| format!("{arg}: option requires an argument"))?;
                    out.shopt_flags.push((name.clone(), arg == "-O"));
                    i += 2;
                }
                "--" => {
                    i += 1;
                    let script = expanded
                        .get(i)
                        .ok_or_else(|| "--: option requires a script".to_string())?;
                    out.script = Some(script.clone());
                    out.positional_params = expanded[i + 1..].to_vec();
                    return Ok(out);
                }
                option if option.starts_with('-') || option.starts_with('+') => {
                    let enabled = option.starts_with('-');
                    let flags = &option[1..];
                    if flags.is_empty()
                        || flags.contains('c')
                        || flags.contains('s')
                        || flags.contains('o')
                    {
                        return Err(format!("{option}: invalid option"));
                    }
                    for flag in flags.chars() {
                        let name = cli_flag_name(flag)
                            .ok_or_else(|| format!("{option}: invalid option"))?;
                        if name == "posix" {
                            out.posix = enabled;
                        }
                        out.shell_flags.push((name.to_string(), enabled));
                    }
                    i += 1;
                }
                script => {
                    out.script = Some(script.to_string());
                    out.positional_params = expanded[i + 1..].to_vec();
                    return Ok(out);
                }
            }
        }
        Ok(out)
    }

    pub fn apply_to_executor(&self, executor: &mut Executor) -> Result<(), String> {
        if self.posix {
            executor.set_env("__RUBASH_POSIX_MODE", "1");
        }
        for (name, enabled) in &self.shell_flags {
            if !executor.is_shell_option(name) {
                return Err(format!("{name}: invalid shell option name"));
            }
            executor.set_shell_option(name, *enabled);
        }
        for (name, enabled) in &self.shopt_flags {
            if !executor.set_shopt_option(name, *enabled) {
                return Err(format!("{name}: invalid shell option name"));
            }
        }
        if self.posix {
            executor.set_shell_option("posix", true);
        }
        if let Some(name) = &self.command_name {
            executor.set_env("__RUBASH_SCRIPT_NAME", name);
        }
        executor.set_positional_params(self.positional_params.clone());
        Ok(())
    }
}

fn cli_flag_name(flag: char) -> Option<&'static str> {
    match flag {
        'a' => Some("allexport"),
        'b' => Some("notify"),
        'e' => Some("errexit"),
        'f' => Some("noglob"),
        'h' => Some("hashall"),
        'n' => Some("noexec"),
        'r' => Some("restricted"),
        'u' => Some("nounset"),
        'v' => Some("verbose"),
        'x' => Some("xtrace"),
        'B' => Some("braceexpand"),
        'C' => Some("noclobber"),
        _ => None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn parses_combined_flags_and_command() {
        let args = vec![
            "-ne".into(),
            "-c".into(),
            "printf ok".into(),
            "demo".into(),
            "x".into(),
        ];
        let parsed = ShellInvocation::parse(&args).unwrap();
        assert_eq!(parsed.command.as_deref(), Some("printf ok"));
        assert!(parsed.shell_flags.contains(&("noexec".into(), true)));
        assert!(parsed.shell_flags.contains(&("errexit".into(), true)));
        assert_eq!(parsed.positional_params, vec!["x"]);
    }
}
