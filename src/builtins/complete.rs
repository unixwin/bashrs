//! complete module.
//!
//! GNU Bash source ownership:
// - builtins/complete.def

use std::collections::{BTreeSet, HashMap};
use std::io::{self, Write};
use std::path::Path;

use crate::builtins::alias::Alias;

const EXECUTION_SUCCESS: i32 = 0;
const EXECUTION_FAILURE: i32 = 1;
const EX_USAGE: i32 = 2;
const DISABLED_BUILTINS: &str = "__RUBASH_DISABLED_BUILTINS";
const EXPORTED_VARS: &str = "__RUBASH_EXPORTED_VARS";
const READONLY_VARS: &str = "__RUBASH_READONLY_VARS";
const ARRAY_VARS: &str = "__RUBASH_ARRAY_VARS";
const ASSOC_VARS: &str = "__RUBASH_ASSOC_VARS";
const SERVICE_NAMES: &str = "__RUBASH_SERVICE_NAMES";
const SHELL_BUILTINS: &[&str] = &[
    ".",
    ":",
    "[",
    "alias",
    "bg",
    "bind",
    "break",
    "builtin",
    "caller",
    "cd",
    "command",
    "compgen",
    "complete",
    "compopt",
    "continue",
    "declare",
    "dirs",
    "disown",
    "echo",
    "enable",
    "eval",
    "exec",
    "exit",
    "export",
    "false",
    "fc",
    "fg",
    "getopts",
    "hash",
    "help",
    "history",
    "jobs",
    "kill",
    "let",
    "local",
    "logout",
    "mapfile",
    "popd",
    "printf",
    "pushd",
    "pwd",
    "read",
    "readarray",
    "readonly",
    "return",
    "set",
    "shift",
    "shopt",
    "source",
    "suspend",
    "test",
    "times",
    "trap",
    "true",
    "type",
    "typeset",
    "ulimit",
    "umask",
    "unalias",
    "unset",
    "wait",
];
const SHELL_KEYWORDS: &[&str] = &[
    "if", "then", "else", "elif", "fi", "case", "esac", "for", "select", "while", "until", "do",
    "done", "in", "function", "time", "{", "}", "!",
];
const READLINE_BINDINGS: &[&str] = &[
    "abort",
    "accept-line",
    "arrow-key-prefix",
    "backward-byte",
    "backward-char",
    "backward-delete-char",
    "backward-kill-line",
    "backward-kill-word",
    "backward-word",
    "beginning-of-history",
    "beginning-of-line",
    "bracketed-paste-begin",
    "call-last-kbd-macro",
    "capitalize-word",
    "character-search",
    "character-search-backward",
    "clear-display",
    "clear-screen",
    "complete",
    "copy-backward-word",
    "copy-forward-word",
    "copy-region-as-kill",
    "delete-char",
    "delete-char-or-list",
    "delete-horizontal-space",
    "digit-argument",
    "do-lowercase-version",
    "downcase-word",
    "dump-functions",
    "dump-macros",
    "dump-variables",
    "emacs-editing-mode",
    "end-kbd-macro",
    "end-of-history",
    "end-of-line",
    "exchange-point-and-mark",
    "execute-named-command",
    "export-completions",
    "fetch-history",
    "forward-backward-delete-char",
    "forward-byte",
    "forward-char",
    "forward-search-history",
    "forward-word",
    "history-search-backward",
    "history-search-forward",
    "history-substring-search-backward",
    "history-substring-search-forward",
    "insert-comment",
    "insert-completions",
    "kill-line",
    "kill-region",
    "kill-whole-line",
    "kill-word",
    "menu-complete",
    "menu-complete-backward",
    "next-history",
    "next-screen-line",
    "non-incremental-forward-search-history",
    "non-incremental-forward-search-history-again",
    "non-incremental-reverse-search-history",
    "non-incremental-reverse-search-history-again",
    "old-menu-complete",
    "operate-and-get-next",
    "overwrite-mode",
    "paste-from-clipboard",
    "possible-completions",
    "previous-history",
    "previous-screen-line",
    "print-last-kbd-macro",
    "quoted-insert",
    "re-read-init-file",
    "redraw-current-line",
    "reverse-search-history",
    "revert-line",
    "self-insert",
    "set-mark",
    "skip-csi-sequence",
    "start-kbd-macro",
    "tab-insert",
    "tilde-expand",
    "transpose-chars",
    "transpose-words",
    "tty-status",
    "undo",
    "universal-argument",
    "unix-filename-rubout",
    "unix-line-discard",
    "unix-word-rubout",
    "upcase-word",
    "vi-append-eol",
    "vi-append-mode",
    "vi-arg-digit",
    "vi-back-to-indent",
    "vi-backward-bigword",
    "vi-backward-word",
    "vi-bWord",
    "vi-change-case",
    "vi-change-char",
    "vi-change-to",
    "vi-char-search",
    "vi-column",
    "vi-complete",
    "vi-delete",
    "vi-delete-to",
    "vi-editing-mode",
    "vi-end-bigword",
    "vi-end-word",
    "vi-eof-maybe",
    "vi-eWord",
    "vi-fetch-history",
    "vi-first-print",
    "vi-forward-bigword",
    "vi-forward-word",
    "vi-fword",
    "vi-goto-mark",
    "vi-insert-beg",
    "vi-insertion-mode",
    "vi-match",
    "vi-movement-mode",
    "vi-next-word",
    "vi-overstrike",
    "vi-overstrike-delete",
    "vi-prev-word",
    "vi-put",
    "vi-redo",
    "vi-replace",
    "vi-rubout",
    "vi-search",
    "vi-search-again",
    "vi-set-mark",
    "vi-subst",
    "vi-tilde-expand",
    "vi-undo",
    "vi-unix-word-rubout",
    "vi-yank-arg",
    "vi-yank-pop",
    "vi-yank-to",
    "yank",
    "yank-last-arg",
    "yank-nth-arg",
    "yank-pop",
];

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum CompletionBuiltin {
    Complete,
    Compgen,
    Compopt,
}

pub fn execute_with_io<E>(
    builtin: CompletionBuiltin,
    args: &[String],
    env_vars: &HashMap<String, String>,
    aliases: &HashMap<String, Alias>,
    function_names: &[String],
    job_names: &[String],
    diagnostic_prefix: &str,
    stdout: &mut E,
    stderr: &mut E,
) -> io::Result<i32>
where
    E: Write,
{
    match builtin {
        CompletionBuiltin::Complete => execute_complete(args, diagnostic_prefix, stdout, stderr),
        CompletionBuiltin::Compgen => execute_compgen(
            args,
            env_vars,
            aliases,
            function_names,
            job_names,
            diagnostic_prefix,
            stdout,
            stderr,
        ),
        CompletionBuiltin::Compopt => execute_compopt(args, diagnostic_prefix, stderr),
    }
}

fn execute_complete<E>(
    args: &[String],
    diagnostic_prefix: &str,
    _stdout: &mut E,
    stderr: &mut E,
) -> io::Result<i32>
where
    E: Write,
{
    Ok(parse_options(
        CompletionBuiltin::Complete,
        args,
        "abcdefgjksuvprDEI",
        "oAGWFCXPS",
        diagnostic_prefix,
        stderr,
    )?
    .status)
}

fn execute_compgen<E>(
    args: &[String],
    env_vars: &HashMap<String, String>,
    aliases: &HashMap<String, Alias>,
    function_names: &[String],
    job_names: &[String],
    diagnostic_prefix: &str,
    stdout: &mut E,
    stderr: &mut E,
) -> io::Result<i32>
where
    E: Write,
{
    let parsed = parse_options(
        CompletionBuiltin::Compgen,
        args,
        "abcdefgjksuv",
        "oAGWFCXPS",
        diagnostic_prefix,
        stderr,
    )?;
    if parsed.status != EXECUTION_SUCCESS {
        return Ok(parsed.status);
    }

    if let Some(wordlist) = parsed.wordlist.as_deref() {
        return write_compgen_matches(wordlist.split_whitespace(), &parsed, stdout);
    }

    if let Some(glob_pattern) = parsed.glob_pattern.as_deref() {
        return match crate::executor::glob::pathname_expand_word(
            glob_pattern,
            &std::collections::HashMap::new(),
        ) {
            crate::executor::glob::PathnameExpansion::Matches(matches) => {
                write_compgen_matches(matches.iter().map(String::as_str), &parsed, stdout)
            }
            crate::executor::glob::PathnameExpansion::NoMatch
            | crate::executor::glob::PathnameExpansion::Fail(_) => Ok(EXECUTION_FAILURE),
        };
    }

    if let Some(action) = parsed.action.as_deref() {
        let candidates = match action {
            "alias" => {
                let candidates = alias_completion_candidates(aliases);
                return write_compgen_matches(
                    candidates.iter().map(String::as_str),
                    &parsed,
                    stdout,
                );
            }
            "arrayvar" => {
                let candidates = array_variable_completion_candidates(env_vars);
                return write_compgen_matches(
                    candidates.iter().map(String::as_str),
                    &parsed,
                    stdout,
                );
            }
            "binding" => READLINE_BINDINGS,
            "builtin" => SHELL_BUILTINS,
            "command" => {
                let candidates = command_completion_candidates(env_vars, aliases, function_names);
                return write_compgen_matches(
                    candidates.iter().map(String::as_str),
                    &parsed,
                    stdout,
                );
            }
            "directory" => {
                let candidates =
                    path_completion_candidates(parsed.word(), PathCompletionKind::Directory);
                return write_compgen_matches(
                    candidates.iter().map(String::as_str),
                    &parsed,
                    stdout,
                );
            }
            "file" => {
                let candidates =
                    path_completion_candidates(parsed.word(), PathCompletionKind::File);
                return write_compgen_matches(
                    candidates.iter().map(String::as_str),
                    &parsed,
                    stdout,
                );
            }
            "disabled" => {
                let candidates = disabled_builtin_completion_candidates(env_vars);
                return write_compgen_matches(
                    candidates.iter().map(String::as_str),
                    &parsed,
                    stdout,
                );
            }
            "enabled" => {
                let candidates = enabled_builtin_completion_candidates(env_vars);
                return write_compgen_matches(
                    candidates.iter().map(String::as_str),
                    &parsed,
                    stdout,
                );
            }
            "export" => {
                let candidates = exported_variable_completion_candidates(env_vars);
                return write_compgen_matches(
                    candidates.iter().map(String::as_str),
                    &parsed,
                    stdout,
                );
            }
            "helptopic" => crate::builtins::help::HELP_TOPICS,
            "hostname" => {
                let candidates = hostname_completion_candidates(env_vars);
                return write_compgen_matches(
                    candidates.iter().map(String::as_str),
                    &parsed,
                    stdout,
                );
            }
            "function" => {
                let candidates = function_completion_candidates(function_names);
                return write_compgen_matches(
                    candidates.iter().map(String::as_str),
                    &parsed,
                    stdout,
                );
            }
            "group" => {
                let candidates = group_completion_candidates(env_vars);
                return write_compgen_matches(
                    candidates.iter().map(String::as_str),
                    &parsed,
                    stdout,
                );
            }
            "job" => {
                let candidates = job_completion_candidates(job_names);
                return write_compgen_matches(
                    candidates.iter().map(String::as_str),
                    &parsed,
                    stdout,
                );
            }
            "keyword" => SHELL_KEYWORDS,
            "readonly" => {
                let candidates = readonly_variable_completion_candidates(env_vars);
                return write_compgen_matches(
                    candidates.iter().map(String::as_str),
                    &parsed,
                    stdout,
                );
            }
            "running" => {
                let candidates = job_completion_candidates(job_names);
                return write_compgen_matches(
                    candidates.iter().map(String::as_str),
                    &parsed,
                    stdout,
                );
            }
            "service" => {
                let candidates = service_completion_candidates(env_vars);
                return write_compgen_matches(
                    candidates.iter().map(String::as_str),
                    &parsed,
                    stdout,
                );
            }
            "signal" => crate::builtins::trap::SIGNALS.as_slice(),
            "shopt" => crate::builtins::shopt::SHOPT_OPTIONS,
            "setopt" => {
                return write_compgen_matches(
                    crate::builtins::set::shell_option_names(),
                    &parsed,
                    stdout,
                );
            }
            "stopped" => return Ok(EXECUTION_SUCCESS),
            "user" => {
                let candidates = user_completion_candidates(env_vars);
                return write_compgen_matches(
                    candidates.iter().map(String::as_str),
                    &parsed,
                    stdout,
                );
            }
            "variable" => {
                let candidates = variable_completion_candidates(env_vars);
                return write_compgen_matches(
                    candidates.iter().map(String::as_str),
                    &parsed,
                    stdout,
                );
            }
            _ => return Ok(EXECUTION_SUCCESS),
        };
        return write_compgen_matches(candidates.iter().copied(), &parsed, stdout);
    }

    Ok(EXECUTION_SUCCESS)
}

#[derive(Clone, Copy)]
enum PathCompletionKind {
    Directory,
    File,
}

fn path_completion_candidates(word: &str, kind: PathCompletionKind) -> Vec<String> {
    let (search_dir, display_prefix) = path_completion_base(word);
    let Ok(entries) = std::fs::read_dir(Path::new(search_dir)) else {
        return Vec::new();
    };

    let mut candidates = Vec::new();
    for entry in entries.flatten() {
        let Ok(file_type) = entry.file_type() else {
            continue;
        };
        if matches!(kind, PathCompletionKind::Directory) && !file_type.is_dir() {
            continue;
        }
        let name = entry.file_name().to_string_lossy().into_owned();
        candidates.push(format!("{display_prefix}{name}"));
    }
    candidates.sort();
    candidates
}

fn path_completion_base(word: &str) -> (&str, &str) {
    let Some(separator_index) = word.rfind(['/', '\\']) else {
        return (".", "");
    };
    if separator_index == 0 {
        return (&word[..1], &word[..1]);
    }
    (&word[..separator_index], &word[..=separator_index])
}

fn enabled_builtin_completion_candidates(env_vars: &HashMap<String, String>) -> Vec<String> {
    let disabled = disabled_builtin_completion_candidates(env_vars);
    SHELL_BUILTINS
        .iter()
        .copied()
        .filter(|name| !disabled.iter().any(|disabled_name| disabled_name == name))
        .map(str::to_string)
        .collect()
}

fn disabled_builtin_completion_candidates(env_vars: &HashMap<String, String>) -> Vec<String> {
    let mut candidates = env_vars
        .get(DISABLED_BUILTINS)
        .map(|value| {
            value
                .split(':')
                .filter(|name| !name.is_empty())
                .map(str::to_string)
                .collect::<Vec<_>>()
        })
        .unwrap_or_default();
    candidates.sort();
    candidates.dedup();
    candidates
}

fn hostname_completion_candidates(env_vars: &HashMap<String, String>) -> Vec<String> {
    let mut candidates = ["HOSTNAME", "COMPUTERNAME"]
        .iter()
        .filter_map(|name| env_vars.get(*name))
        .filter(|value| !value.is_empty())
        .cloned()
        .collect::<Vec<_>>();
    candidates.sort();
    candidates.dedup();
    candidates
}

fn user_completion_candidates(env_vars: &HashMap<String, String>) -> Vec<String> {
    let mut candidates = ["USER", "LOGNAME", "USERNAME"]
        .iter()
        .filter_map(|name| env_vars.get(*name))
        .filter(|value| !value.is_empty())
        .cloned()
        .collect::<Vec<_>>();
    candidates.sort();
    candidates.dedup();
    candidates
}

fn group_completion_candidates(env_vars: &HashMap<String, String>) -> Vec<String> {
    let mut candidates = ["GROUP", "GROUPNAME", "USERDOMAIN"]
        .iter()
        .filter_map(|name| env_vars.get(*name))
        .filter(|value| !value.is_empty())
        .cloned()
        .collect::<Vec<_>>();
    candidates.sort();
    candidates.dedup();
    candidates
}

fn service_completion_candidates(env_vars: &HashMap<String, String>) -> Vec<String> {
    let mut candidates = ["SERVICE", "SERVICENAME"]
        .iter()
        .filter_map(|name| env_vars.get(*name))
        .filter(|value| !value.is_empty())
        .cloned()
        .collect::<Vec<_>>();
    candidates.extend(marked_completion_names(env_vars, SERVICE_NAMES));
    candidates.sort();
    candidates.dedup();
    candidates
}

fn variable_completion_candidates(env_vars: &HashMap<String, String>) -> Vec<String> {
    let mut candidates: Vec<String> = env_vars.keys().cloned().collect();
    candidates.sort();
    candidates.dedup();
    candidates
}

fn array_variable_completion_candidates(env_vars: &HashMap<String, String>) -> Vec<String> {
    let mut candidates = BTreeSet::new();
    candidates.extend(marked_completion_names(env_vars, ARRAY_VARS));
    candidates.extend(marked_completion_names(env_vars, ASSOC_VARS));
    candidates.into_iter().collect()
}

fn exported_variable_completion_candidates(env_vars: &HashMap<String, String>) -> Vec<String> {
    let mut candidates = marked_completion_names(env_vars, EXPORTED_VARS);
    candidates.sort();
    candidates.dedup();
    candidates
}

fn readonly_variable_completion_candidates(env_vars: &HashMap<String, String>) -> Vec<String> {
    let mut candidates = marked_completion_names(env_vars, READONLY_VARS);
    candidates.sort();
    candidates.dedup();
    candidates
}

fn marked_completion_names(env_vars: &HashMap<String, String>, key: &str) -> Vec<String> {
    env_vars
        .get(key)
        .map(|value| {
            value
                .split('\x1f')
                .filter(|name| !name.is_empty())
                .map(str::to_string)
                .collect()
        })
        .unwrap_or_default()
}

fn alias_completion_candidates(aliases: &HashMap<String, Alias>) -> Vec<String> {
    let mut candidates: Vec<String> = aliases.keys().cloned().collect();
    candidates.sort();
    candidates
}

fn function_completion_candidates(function_names: &[String]) -> Vec<String> {
    let mut candidates = function_names.to_vec();
    candidates.sort();
    candidates.dedup();
    candidates
}

fn job_completion_candidates(job_names: &[String]) -> Vec<String> {
    let mut candidates = job_names.to_vec();
    candidates.sort();
    candidates.dedup();
    candidates
}

fn command_completion_candidates(
    env_vars: &HashMap<String, String>,
    aliases: &HashMap<String, Alias>,
    function_names: &[String],
) -> Vec<String> {
    let mut candidates = BTreeSet::new();
    candidates.extend(SHELL_BUILTINS.iter().map(|name| (*name).to_string()));
    candidates.extend(SHELL_KEYWORDS.iter().map(|name| (*name).to_string()));
    candidates.extend(aliases.keys().cloned());
    candidates.extend(function_names.iter().cloned());
    candidates.extend(path_command_completion_candidates(env_vars));
    candidates.into_iter().collect()
}

fn path_command_completion_candidates(env_vars: &HashMap<String, String>) -> Vec<String> {
    let Some(path_value) = env_vars.get("PATH") else {
        return Vec::new();
    };

    let mut candidates = Vec::new();
    for dir in std::env::split_paths(path_value) {
        let Ok(entries) = std::fs::read_dir(dir) else {
            continue;
        };
        for entry in entries.flatten() {
            let Ok(file_type) = entry.file_type() else {
                continue;
            };
            if file_type.is_file() {
                candidates.push(entry.file_name().to_string_lossy().into_owned());
            }
        }
    }
    candidates
}

fn write_compgen_matches<'a, I, E>(
    candidates: I,
    parsed: &ParsedCompletionOptions,
    stdout: &mut E,
) -> io::Result<i32>
where
    I: IntoIterator<Item = &'a str>,
    E: Write,
{
    let word = parsed.word();
    let mut matched = false;
    for candidate in candidates {
        if candidate.starts_with(word) {
            matched = true;
            if parsed.filter_excludes(candidate) {
                continue;
            }
            writeln!(
                stdout,
                "{}{}{}",
                parsed.prefix.as_deref().unwrap_or_default(),
                candidate,
                parsed.suffix.as_deref().unwrap_or_default()
            )?;
        }
    }
    Ok(if matched {
        EXECUTION_SUCCESS
    } else {
        EXECUTION_FAILURE
    })
}

fn execute_compopt<E>(args: &[String], diagnostic_prefix: &str, stderr: &mut E) -> io::Result<i32>
where
    E: Write,
{
    let status = parse_compopt_options(args, diagnostic_prefix, stderr)?;
    if status != EXECUTION_SUCCESS {
        return Ok(status);
    }

    writeln!(
        stderr,
        "{diagnostic_prefix}compopt: not currently executing completion function"
    )?;
    Ok(EXECUTION_FAILURE)
}

fn parse_options<E>(
    builtin: CompletionBuiltin,
    args: &[String],
    flag_options: &str,
    arg_options: &str,
    diagnostic_prefix: &str,
    stderr: &mut E,
) -> io::Result<ParsedCompletionOptions>
where
    E: Write,
{
    let name = builtin.name();
    let mut index = 0;
    let mut parsed = ParsedCompletionOptions::default();
    while let Some(arg) = args.get(index) {
        if arg == "--" {
            index += 1;
            break;
        }
        if !arg.starts_with('-') || arg == "-" {
            break;
        }

        let mut chars = arg[1..].chars().peekable();
        while let Some(option) = chars.next() {
            if flag_options.contains(option) {
                parsed.set_flag_option(option);
                continue;
            }
            if arg_options.contains(option) {
                let inline_arg = chars.peek().is_some();
                let value = if inline_arg {
                    chars.collect::<String>()
                } else {
                    index += 1;
                    let Some(value) = args.get(index) else {
                        writeln!(
                            stderr,
                            "{diagnostic_prefix}{name}: -{option}: option requires an argument"
                        )?;
                        write_usage(builtin, stderr)?;
                        return Ok(ParsedCompletionOptions::status(EX_USAGE));
                    };
                    value.clone()
                };
                parsed.set_option_arg(option, value);
                if inline_arg {
                    break;
                }
                break;
            }

            writeln!(
                stderr,
                "{diagnostic_prefix}{name}: -{option}: invalid option"
            )?;
            write_usage(builtin, stderr)?;
            return Ok(ParsedCompletionOptions::status(EX_USAGE));
        }
        index += 1;
    }

    parsed.operands = args[index..].to_vec();
    Ok(parsed)
}

#[derive(Default)]
struct ParsedCompletionOptions {
    status: i32,
    action: Option<String>,
    glob_pattern: Option<String>,
    wordlist: Option<String>,
    filter_pattern: Option<String>,
    prefix: Option<String>,
    suffix: Option<String>,
    operands: Vec<String>,
}

impl ParsedCompletionOptions {
    fn status(status: i32) -> Self {
        Self {
            status,
            ..Self::default()
        }
    }

    fn set_option_arg(&mut self, option: char, value: String) {
        match option {
            'A' => self.action = Some(value),
            'G' => self.glob_pattern = Some(value),
            'W' => self.wordlist = Some(value),
            'X' => self.filter_pattern = Some(value),
            'P' => self.prefix = Some(value),
            'S' => self.suffix = Some(value),
            _ => {}
        }
    }

    fn set_flag_option(&mut self, option: char) {
        match option {
            'a' => self.action = Some("alias".to_string()),
            'b' => self.action = Some("builtin".to_string()),
            'c' => self.action = Some("command".to_string()),
            'd' => self.action = Some("directory".to_string()),
            'f' => self.action = Some("file".to_string()),
            'g' => self.action = Some("group".to_string()),
            'j' => self.action = Some("job".to_string()),
            'k' => self.action = Some("keyword".to_string()),
            's' => self.action = Some("service".to_string()),
            'u' => self.action = Some("user".to_string()),
            'v' => self.action = Some("variable".to_string()),
            _ => {}
        }
    }

    fn word(&self) -> &str {
        self.operands
            .first()
            .map(String::as_str)
            .unwrap_or_default()
    }

    fn filter_excludes(&self, candidate: &str) -> bool {
        let Some(filter_pattern) = self.filter_pattern.as_deref() else {
            return false;
        };
        if let Some(pattern) = filter_pattern.strip_prefix('!') {
            !crate::executor::conditional::shell_pattern_matches(pattern, candidate)
        } else {
            crate::executor::conditional::shell_pattern_matches(filter_pattern, candidate)
        }
    }
}

fn parse_compopt_options<E>(
    args: &[String],
    diagnostic_prefix: &str,
    stderr: &mut E,
) -> io::Result<i32>
where
    E: Write,
{
    let mut index = 0;
    while let Some(arg) = args.get(index) {
        if arg == "--" {
            break;
        }
        if !arg.starts_with('-') && !arg.starts_with("+o") || arg == "-" {
            break;
        }

        if let Some(rest) = arg.strip_prefix("+o") {
            if rest.is_empty() {
                index += 1;
                if args.get(index).is_none() {
                    writeln!(
                        stderr,
                        "{diagnostic_prefix}compopt: +o: option requires an argument"
                    )?;
                    write_usage(CompletionBuiltin::Compopt, stderr)?;
                    return Ok(EX_USAGE);
                }
            }
            index += 1;
            continue;
        }

        let mut chars = arg[1..].chars().peekable();
        while let Some(option) = chars.next() {
            match option {
                'D' | 'E' | 'I' => {}
                'o' => {
                    if chars.peek().is_none() {
                        index += 1;
                        if args.get(index).is_none() {
                            writeln!(
                                stderr,
                                "{diagnostic_prefix}compopt: -o: option requires an argument"
                            )?;
                            write_usage(CompletionBuiltin::Compopt, stderr)?;
                            return Ok(EX_USAGE);
                        }
                    }
                    break;
                }
                other => {
                    writeln!(
                        stderr,
                        "{diagnostic_prefix}compopt: -{other}: invalid option"
                    )?;
                    write_usage(CompletionBuiltin::Compopt, stderr)?;
                    return Ok(EX_USAGE);
                }
            }
        }
        index += 1;
    }

    Ok(EXECUTION_SUCCESS)
}

impl CompletionBuiltin {
    fn name(self) -> &'static str {
        match self {
            CompletionBuiltin::Complete => "complete",
            CompletionBuiltin::Compgen => "compgen",
            CompletionBuiltin::Compopt => "compopt",
        }
    }
}

fn write_usage<E>(builtin: CompletionBuiltin, stderr: &mut E) -> io::Result<()>
where
    E: Write,
{
    let usage = match builtin {
        CompletionBuiltin::Complete => {
            "complete: usage: complete [-abcdefgjksuv] [-pr] [-DEI] [-o option] [-A action] [-G globpat] [-W wordlist] [-F function] [-C command] [-X filterpat] [-P prefix] [-S suffix] [name ...]"
        }
        CompletionBuiltin::Compgen => {
            "compgen: usage: compgen [-abcdefgjksuv] [-o option] [-A action] [-G globpat] [-W wordlist] [-F function] [-C command] [-X filterpat] [-P prefix] [-S suffix] [word]"
        }
        CompletionBuiltin::Compopt => {
            "compopt: usage: compopt [-o|+o option] [-DEI] [name ...]"
        }
    };
    writeln!(stderr, "{usage}")
}
