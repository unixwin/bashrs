use crate::executor::path::{shell_path_entries, shell_path_to_windows};
use crate::executor::Executor;
use std::path::PathBuf;

pub(super) struct SourceInvocation<'a> {
    pub(super) filename: &'a str,
    pub(super) args: &'a [String],
    pub(super) path: Option<String>,
}

pub(super) enum SourceParseError {
    MissingFilename,
    MissingPathArgument,
    InvalidOption(char),
}

impl<'a> SourceInvocation<'a> {
    pub(super) fn parse(args: &'a [String]) -> Result<Self, SourceParseError> {
        let mut index = 0;
        let mut path = None;

        while let Some(arg) = args.get(index) {
            if arg == "--" {
                index += 1;
                break;
            }

            if arg == "-p" {
                let Some(value) = args.get(index + 1) else {
                    return Err(SourceParseError::MissingPathArgument);
                };
                path = Some(if value.is_empty() {
                    ".".to_string()
                } else {
                    value.clone()
                });
                index += 2;
                continue;
            }

            if let Some(option) = invalid_option(arg) {
                return Err(SourceParseError::InvalidOption(option));
            }

            break;
        }

        let Some(filename) = args.get(index).map(String::as_str) else {
            return Err(SourceParseError::MissingFilename);
        };

        Ok(Self {
            filename,
            args: &args[index + 1..],
            path,
        })
    }

    pub(super) fn resolve_path(&self, executor: &Executor) -> Option<PathBuf> {
        if let Some(path) = &self.path {
            return source_path_search(path, self.filename, executor);
        }

        if should_search_source_path(executor, self.filename) {
            if let Some(path) = executor
                .get_env("PATH")
                .and_then(|path| source_path_search(path, self.filename, executor))
            {
                return Some(path);
            }
        }

        // GNU source.def: the current-directory fallback happens only when
        // source_searches_cwd is set, and general.c posix_initialize() clears
        // that flag in POSIX mode.  A plain name there is looked up via $PATH
        // only (source_uses_path is forced on); names with a directory
        // component are still opened directly (absolute_program path).
        let plain_name = !self.filename.contains('/') && !self.filename.contains('\\');
        if plain_name && executor.get_env("__RUBASH_POSIX_MODE") == Some("1") {
            return None;
        }

        let source_path = shell_path_to_windows(self.filename, executor.env_vars());
        source_path.exists().then_some(source_path)
    }
}

fn invalid_option(arg: &str) -> Option<char> {
    let option = arg.strip_prefix('-')?;
    if option.is_empty() {
        return None;
    }
    option.chars().next()
}

fn should_search_source_path(executor: &Executor, filename: &str) -> bool {
    crate::builtins::shopt::sourcepath_enabled()
        && !filename.contains('/')
        && !filename.contains('\\')
        && executor.get_env("PATH").is_some()
}

fn source_path_search(path: &str, filename: &str, executor: &Executor) -> Option<PathBuf> {
    // The PATH separator is platform-specific: Windows uses ';' with
    // drive-letter colons, POSIX uses ':'.  split_shell_path understands
    // both, so it must be used here instead of a raw `path.split(':')`.
    // A raw POSIX split of a Windows PATH produced garbage `\\...` UNC-ish
    // candidates whose is_file() triggered slow UNC name resolution (~2s per
    // entry, ~17s for a full PATH), stalling `.`/source lookups for a missing
    // file (builtins.tests TIMEOUT).
    //
    // Empty components mean the current directory (GNU findcmd.c behavior).
    for entry in shell_path_entries(path) {
        let candidate = if entry.is_empty() || entry == "." {
            PathBuf::from(filename)
        } else {
            let mut base = shell_path_to_windows(&entry, executor.env_vars());
            base.push(filename);
            base
        };

        if candidate.is_file() {
            return Some(candidate);
        }
    }

    None
}
