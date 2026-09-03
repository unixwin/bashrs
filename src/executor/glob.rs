//! Pathname expansion (globbing) for command words.

use std::path::Path;

use crate::executor::path::{shell_directory_entries, shell_path_to_windows};

pub(crate) enum PathnameExpansion {
    Matches(Vec<String>),
    NoMatch,
    Fail(String),
}

/// Check if a shopt option is enabled.
fn shopt_enabled(env_vars: &std::collections::HashMap<String, String>, name: &str) -> bool {
    crate::builtins::shopt::option_enabled(env_vars, name)
}

/// Check if a word contains glob or extglob pattern characters.
fn contains_glob_or_extglob(word: &str) -> bool {
    let chars: Vec<char> = word.chars().collect();
    let mut index = 0usize;
    while index < chars.len() {
        let ch = chars[index];
        if ch == '\\' || ch == '\x11' {
            index += 2;
            continue;
        }
        if matches!(ch, '*' | '?' | '[') {
            return true;
        }
        if matches!(ch, '@' | '+' | '!') && chars.get(index + 1) == Some(&'(') {
            return true;
        }
        index += 1;
    }
    false
}

fn contains_extglob(pattern: &str) -> bool {
    let chars: Vec<char> = pattern.chars().collect();
    chars
        .iter()
        .enumerate()
        .any(|(i, ch)| matches!(ch, '@' | '*' | '+' | '?' | '!') && chars.get(i + 1) == Some(&'('))
}

/// Expand glob patterns (* ? [...]) in a word against the filesystem.
pub(crate) fn pathname_expand_word(
    word: &str,
    env_vars: &std::collections::HashMap<String, String>,
) -> PathnameExpansion {
    if word.is_empty() {
        return PathnameExpansion::NoMatch;
    }
    if word.starts_with('"') || word.starts_with('\'') {
        return PathnameExpansion::NoMatch;
    }
    if !contains_glob_or_extglob(word) {
        return PathnameExpansion::NoMatch;
    }
    if word.contains('{') || word.contains('}') {
        return PathnameExpansion::NoMatch;
    }
    if crate::builtins::set::shell_option_enabled(env_vars, "noglob") {
        return PathnameExpansion::NoMatch;
    }

    let nullglob = shopt_enabled(env_vars, "nullglob");
    let failglob = shopt_enabled(env_vars, "failglob");
    let dotglob = shopt_enabled(env_vars, "dotglob");
    let globskipdots = shopt_enabled(env_vars, "globskipdots");
    let nocaseglob = shopt_enabled(env_vars, "nocaseglob");
    let globstar = shopt_enabled(env_vars, "globstar");
    let extglob = shopt_enabled(env_vars, "extglob");

    if word.contains("**") && globstar {
        return globstar_expand(
            word,
            nullglob,
            failglob,
            nocaseglob,
            dotglob,
            globskipdots,
            env_vars,
        );
    }

    if word.contains('/') {
        return pathname_expand_segments(
            word,
            nullglob,
            failglob,
            nocaseglob,
            dotglob,
            globskipdots,
            extglob,
            env_vars,
        );
    }

    let (dir_path, pattern) = match word.rsplit_once('/') {
        Some((d, p)) => (d.to_string(), p),
        None => (".".to_string(), word.as_ref()),
    };
    let include_dotfiles =
        dotglob || pattern.starts_with('.') || globignore_assigned(env_vars);
    let entries = match shell_directory_entries(&dir_path, env_vars) {
        Ok(entries) => entries,
        Err(_) => return unmatched_expansion(word, nullglob, failglob),
    };
    let mut names = synthetic_dot_names(pattern, globskipdots);
    names.extend(entries.into_iter().map(|entry| entry.name));
    let matches: Vec<String> = names
        .into_iter()
        .filter_map(|name| {
            if !include_dotfiles && name.starts_with('.') {
                return None;
            }
            let matched = pathname_pattern_matches(pattern, &name, nocaseglob, extglob);
            if matched {
                if dir_path == "." {
                    Some(name)
                } else {
                    Some(format!("{dir_path}/{name}"))
                }
            } else {
                None
            }
        })
        .collect();
    let mut matches = apply_globignore(matches, env_vars);
    if matches.is_empty() {
        return unmatched_expansion(word, nullglob, failglob);
    }
    matches.sort();
    PathnameExpansion::Matches(matches)
}

fn pathname_expand_segments(
    word: &str,
    nullglob: bool,
    failglob: bool,
    nocaseglob: bool,
    dotglob: bool,
    globskipdots: bool,
    extglob: bool,
    env_vars: &std::collections::HashMap<String, String>,
) -> PathnameExpansion {
    let parts: Vec<&str> = word.split('/').collect();
    let mut prefixes = if word.starts_with('/') {
        vec!["/".to_string()]
    } else {
        vec![String::new()]
    };
    let mut saw_pattern = false;
    let globignore_assigned = globignore_assigned(env_vars);

    for (index, part) in parts.iter().enumerate() {
        if part.is_empty() {
            continue;
        }
        let is_last = index == parts.len() - 1;
        let part_has_pattern = contains_glob_or_extglob(part);
        saw_pattern |= part_has_pattern;
        let mut next = Vec::new();

        for prefix in &prefixes {
            if part_has_pattern {
                let dir = if prefix.is_empty() {
                    "."
                } else {
                    prefix.as_str()
                };
                let entries = match shell_directory_entries(dir, env_vars) {
                    Ok(entries) => entries,
                    Err(_) => continue,
                };
                let pattern = unescape_glob_pattern_literals(part);
                let include_dotfiles = dotglob
                    || pattern.starts_with('.')
                    || globignore_assigned;
                let mut names = synthetic_dot_names(&pattern, globskipdots);
                names.extend(entries.into_iter().map(|entry| entry.name));
                for name in names {
                    if !include_dotfiles && name.starts_with('.') {
                        continue;
                    }
                    if pathname_pattern_matches(&pattern, &name, nocaseglob, extglob) {
                        next.push(join_path_segment(prefix, &name));
                    }
                }
            } else {
                let literal_part = if saw_pattern {
                    unescape_glob_pattern_literals(part)
                } else {
                    (*part).to_string()
                };
                let candidate = join_path_segment(prefix, &literal_part);
                if !is_last || !saw_pattern || shell_path_to_windows(&candidate, env_vars).exists()
                {
                    next.push(candidate);
                }
            }
        }

        prefixes = next;
        if prefixes.is_empty() {
            break;
        }
    }

    let mut prefixes = apply_globignore(prefixes, env_vars);
    if prefixes.is_empty() {
        return unmatched_expansion(word, nullglob, failglob);
    }
    prefixes.sort();
    PathnameExpansion::Matches(prefixes)
}

fn join_path_segment(prefix: &str, segment: &str) -> String {
    if prefix.is_empty() {
        segment.to_string()
    } else if prefix == "/" {
        format!("/{segment}")
    } else {
        format!("{prefix}/{segment}")
    }
}

fn unescape_glob_pattern_literals(pattern: &str) -> String {
    let mut output = String::new();
    let mut chars = pattern.chars().peekable();
    while let Some(ch) = chars.next() {
        if ch == '\\' {
            match chars.peek().copied() {
                Some(next @ ('*' | '?' | '[' | ']' | '\\')) => {
                    output.push('\\');
                    output.push(next);
                    chars.next();
                }
                Some(next) => {
                    output.push(next);
                    chars.next();
                }
                None => output.push('\\'),
            }
        } else {
            output.push(ch);
        }
    }
    output
}

fn pathname_pattern_matches(pattern: &str, word: &str, nocaseglob: bool, extglob: bool) -> bool {
    // Bash's CTLESC marker (0x81) quotes a filename character from pathname
    // expansion. Treat it as an indivisible literal, so `?` and `*` cannot
    // consume it while a direct literal redirect/read remains valid.
    if cfg!(windows) && word.contains('\u{81}') && contains_glob_or_extglob(pattern) {
        return false;
    }
    if extglob && contains_extglob(pattern) {
        extglob_pattern_matches(pattern, word, nocaseglob)
    } else if nocaseglob {
        case_pattern_matches_nocase(pattern, word)
    } else {
        super::case_pattern_matches(pattern, word)
    }
}

fn case_pattern_matches_nocase(pattern: &str, word: &str) -> bool {
    let pattern_lower = pattern.to_lowercase();
    let word_lower = word.to_lowercase();
    super::case_pattern_matches(&pattern_lower, &word_lower)
}

fn extglob_pattern_matches(pattern: &str, word: &str, nocaseglob: bool) -> bool {
    if nocaseglob {
        let pattern_lower = pattern.to_lowercase();
        let word_lower = word.to_lowercase();
        super::conditional::extglob_case_pattern_matches(&pattern_lower, &word_lower)
    } else {
        super::conditional::extglob_case_pattern_matches(pattern, word)
    }
}

fn globstar_expand(
    word: &str,
    nullglob: bool,
    failglob: bool,
    nocaseglob: bool,
    dotglob: bool,
    globskipdots: bool,
    env_vars: &std::collections::HashMap<String, String>,
) -> PathnameExpansion {
    let parts: Vec<&str> = word.split("**").collect();
    if parts.len() != 2 {
        return PathnameExpansion::NoMatch;
    }
    let prefix = parts[0];
    // GNU globstar forms (parse.y/glob.c GLOBSTAR): a bare `**` matches every
    // file and directory at any depth (empty remainder), `**/` matches
    // directories only with a trailing slash in the output, and `**/pattern`
    // applies the pattern at every depth. Symlinked directories are listed as
    // entries but never recursed into (loop avoidance).
    let raw_suffix = parts[1];
    let dirs_only = raw_suffix == "/";
    let match_all = raw_suffix.is_empty();
    let suffix = if dirs_only { "" } else { raw_suffix.trim_start_matches('/') };

    let logical_base_dir = if prefix.is_empty() {
        ".".to_string()
    } else if prefix == "/" {
        "/".to_string()
    } else {
        prefix.trim_end_matches('/').to_string()
    };
    let physical_base_dir = shell_path_to_windows(&logical_base_dir, env_vars);

    let mut matches = Vec::new();
    collect_globstar_matches(
        &logical_base_dir,
        &physical_base_dir,
        suffix,
        match_all,
        dirs_only,
        &mut matches,
        nocaseglob,
        dotglob,
        globskipdots,
        env_vars,
    );

    let mut matches = apply_globignore(matches, env_vars);
    if matches.is_empty() {
        return unmatched_expansion(word, nullglob, failglob);
    }
    matches.sort();
    PathnameExpansion::Matches(matches)
}

fn unmatched_expansion(word: &str, nullglob: bool, failglob: bool) -> PathnameExpansion {
    if failglob {
        PathnameExpansion::Fail(word.to_string())
    } else if nullglob {
        PathnameExpansion::Matches(Vec::new())
    } else {
        PathnameExpansion::NoMatch
    }
}

fn collect_globstar_matches(
    logical_dir: &str,
    physical_dir: &Path,
    suffix: &str,
    match_all: bool,
    dirs_only: bool,
    matches: &mut Vec<String>,
    nocaseglob: bool,
    dotglob: bool,
    globskipdots: bool,
    env_vars: &std::collections::HashMap<String, String>,
) {
    let entries = match shell_directory_entries(logical_dir, env_vars) {
        Ok(entries) => entries,
        Err(_) => return,
    };
    let mut names = synthetic_dot_names(suffix, globskipdots);
    names.extend(entries.iter().map(|entry| entry.name.clone()));
    let include_dotfiles = dotglob
        || suffix.starts_with('.')
        || globignore_patterns(env_vars).is_some();
    for name in names {
        if name.starts_with('.') && !include_dotfiles {
            continue;
        }
        let physical_path = entries
            .iter()
            .find(|entry| entry.name == name)
            .map(|entry| entry.path.clone())
            .unwrap_or_else(|| physical_dir.join(&name));
        // GNU globstar output uses plain relative paths: the cwd-relative
        // base `.` contributes no `./` prefix to matched entries (glob.c
        // globstar expansion - `echo **` yields `a a/aa b` not `./a ./a/aa
        // ./b`). Keep `.` for the directory read but normalize it away in
        // the joined logical path.
        let join_prefix = if logical_dir == "." { "" } else { logical_dir };
        let logical_path = join_path_segment(join_prefix, &name);
        let is_dir = physical_path.is_dir();
        let matched = if match_all {
            true
        } else if dirs_only {
            is_dir
        } else if nocaseglob {
            case_pattern_matches_nocase(suffix, &name)
        } else {
            super::case_pattern_matches(suffix, &name)
        };
        if matched {
            let output = if dirs_only {
                format!("{}/", logical_path)
            } else {
                logical_path.clone()
            };
            matches.push(output);
        }
        // Recurse into real directories only: GNU never follows symlinked
        // directories during ** recursion (loop avoidance; the symlink
        // itself still appears as a matched entry above).
        let is_symlink = std::fs::symlink_metadata(&physical_path)
            .map(|meta| meta.file_type().is_symlink())
            .unwrap_or(false);
        if is_dir && !is_symlink && name != "." && name != ".." {
            collect_globstar_matches(
                &logical_path,
                &physical_path,
                suffix,
                match_all,
                dirs_only,
                matches,
                nocaseglob,
                dotglob,
                globskipdots,
                env_vars,
            );
        }
    }
}

/// Returns the GLOBIGNORE pattern list when the variable is set to a
/// non-null value. A null GLOBIGNORE still enables the dotfile side effect
/// (see globignore_assigned) but contributes no filter patterns.
fn globignore_patterns(
    env_vars: &std::collections::HashMap<String, String>,
) -> Option<Vec<String>> {
    let value = env_vars.get("GLOBIGNORE")?;
    if value.is_empty() {
        return None;
    }
    Some(
        value
            .split(':')
            .filter(|pattern| !pattern.is_empty())
            .map(str::to_string)
            .collect(),
    )
}

/// True when GLOBIGNORE has been assigned at all (set, even to the null
/// string). WSL GNU 5.2.21 probe: after `GLOBIGNORE=` dotfiles still match
/// (the assignment enables the dotglob side effect) while only unsetting
/// restores the default; pattern filtering itself requires a non-null
/// value.
fn globignore_assigned(env_vars: &std::collections::HashMap<String, String>) -> bool {
    env_vars.contains_key("GLOBIGNORE")
}

/// Filters a collected pathname-expansion match list by GLOBIGNORE (bash
/// glob.c ignorable()): when GLOBIGNORE is set non-null, a match whose
/// basename matches any ignore pattern is removed, and `.` / `..` are
/// always removed. Assigning GLOBIGNORE also enables dotfile matching (the
/// dotglob side effect), which the callers handle at collection time.
fn apply_globignore(
    matches: Vec<String>,
    env_vars: &std::collections::HashMap<String, String>,
) -> Vec<String> {
    let Some(patterns) = globignore_patterns(env_vars) else {
        return matches;
    };
    matches
        .into_iter()
        .filter(|name| {
            let base = name.rsplit('/').next().unwrap_or(name);
            base != "." && base != ".."
                && !patterns
                    .iter()
                    .any(|pattern| super::case_pattern_matches(pattern, base))
        })
        .collect()
}

fn synthetic_dot_names(pattern: &str, globskipdots: bool) -> Vec<String> {
    if globskipdots || !pattern.starts_with('.') {
        Vec::new()
    } else {
        vec![".".to_string(), "..".to_string()]
    }
}

#[cfg(test)]
mod tests {
    #[cfg(windows)]
    use super::{pathname_expand_word, pathname_pattern_matches, PathnameExpansion};
    #[cfg(windows)]
    use std::collections::HashMap;

    #[cfg(windows)]
    #[test]
    fn logical_root_glob_reads_backing_directory_and_returns_logical_names() {
        let root = std::env::temp_dir().join("rubash-logical-root-glob");
        let _ = std::fs::remove_dir_all(&root);
        std::fs::create_dir_all(root.join("etc")).unwrap();
        std::fs::write(root.join("etc").join("config"), "value").unwrap();

        let mut env_vars = HashMap::new();
        env_vars.insert(
            "RUBASH_ROOT".to_string(),
            root.to_string_lossy().to_string(),
        );

        let PathnameExpansion::Matches(matches) = pathname_expand_word("/etc/*", &env_vars) else {
            panic!("logical root glob did not produce matches");
        };
        assert_eq!(matches, vec!["/etc/config".to_string()]);

        let _ = std::fs::remove_dir_all(root);
    }

    #[cfg(windows)]
    #[test]
    fn ctlesc_filename_is_not_consumed_by_pathname_wildcards() {
        let marker_name = "uni\u{81}code";

        assert!(pathname_pattern_matches(
            marker_name,
            marker_name,
            false,
            false
        ));
        assert!(!pathname_pattern_matches(
            "uni?code",
            marker_name,
            false,
            false
        ));
        assert!(!pathname_pattern_matches(
            "uni*code",
            marker_name,
            false,
            false
        ));
    }
}
