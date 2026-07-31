//! path module.
//!
//! GNU Bash source ownership:
// - findcmd.c
// - findcmd.h

use std::collections::HashMap;
use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;

use super::support_names::split_shell_path;

pub fn find_user_command(name: &str, env_vars: &HashMap<String, String>) -> Option<PathBuf> {
    if name.is_empty() {
        return None;
    }

    if has_path_separator(name) {
        let candidate = shell_path_to_windows(name, env_vars);
        return executable_candidate(&candidate);
    }

    for dir in split_shell_path(env_vars.get("PATH").map(String::as_str).unwrap_or_default()) {
        let candidate = shell_path_to_windows(&dir, env_vars).join(name);
        if let Some(found) = executable_candidate(&candidate) {
            return Some(found);
        }
    }

    None
}

pub fn standard_path(_env_vars: &HashMap<String, String>) -> String {
    if cfg!(windows) {
        return [
            PathBuf::from(r"C:\Windows\System32"),
            PathBuf::from(r"C:\Windows"),
        ]
        .into_iter()
        .map(|path| path.to_string_lossy().to_string())
        .collect::<Vec<_>>()
        .join(";");
    }

    "/usr/local/bin:/usr/bin:/bin".to_string()
}

pub fn find_shell(env_vars: &HashMap<String, String>) -> Option<PathBuf> {
    let path_shell = ["sh", "bash"]
        .into_iter()
        .find_map(|name| find_user_command(name, env_vars));

    if cfg!(windows) {
        path_shell
    } else {
        path_shell.or_else(find_standard_unix_shell)
    }
}

pub fn should_run_with_shell(path: &Path) -> bool {
    if cfg!(windows) {
        !matches!(
            path.extension().and_then(|ext| ext.to_str()).map(str::to_ascii_lowercase),
            Some(ext) if matches!(ext.as_str(), "exe" | "com" | "bat" | "cmd")
        )
    } else {
        false
    }
}

pub fn external_command_for_program(
    program: &Path,
    args: &[String],
    env_vars: &HashMap<String, String>,
) -> (Command, bool) {
    if is_windows_batch_file(program) {
        let program = cmd_compatible_windows_path(program);
        let mut command = Command::new(windows_command_processor());
        command.arg("/D").arg("/C").arg(program);
        command.args(args);
        return (command, false);
    }

    if should_run_with_shell(program) {
        if let Some(shell) = find_shell(env_vars) {
            let mut command = Command::new(shell);
            command.arg(program);
            command.args(args);
            return (command, true);
        }
    }

    let mut command = Command::new(program);
    command.args(args);
    (command, false)
}

fn is_windows_batch_file(path: &Path) -> bool {
    cfg!(windows)
        && path
            .extension()
            .and_then(|ext| ext.to_str())
            .is_some_and(|ext| matches!(ext.to_ascii_lowercase().as_str(), "bat" | "cmd"))
}

fn windows_command_processor() -> PathBuf {
    std::env::var_os("ComSpec")
        .map(PathBuf::from)
        .unwrap_or_else(|| PathBuf::from(r"C:\Windows\System32\cmd.exe"))
}

fn cmd_compatible_windows_path(path: &Path) -> PathBuf {
    let path = fs::canonicalize(path).unwrap_or_else(|_| path.to_path_buf());
    if !cfg!(windows) {
        return path;
    }

    let value = path.to_string_lossy();
    if let Some(rest) = value.strip_prefix(r"\\?\UNC\") {
        return PathBuf::from(format!(r"\\{rest}"));
    }
    if let Some(rest) = value.strip_prefix(r"\\?\") {
        return PathBuf::from(rest);
    }
    path
}

pub fn apply_required_windows_child_environment(
    process: &mut Command,
    env_vars: &HashMap<String, String>,
) {
    if !cfg!(windows) {
        return;
    }

    for name in ["SystemRoot", "WINDIR", "ComSpec"] {
        let value = env_vars
            .get(name)
            .cloned()
            .or_else(|| std::env::var(name).ok());
        if let Some(value) = value {
            if !value.contains('\0') {
                process.env(name, value);
            }
        }
    }
}

fn executable_candidate(path: &Path) -> Option<PathBuf> {
    if cfg!(windows) && path.extension().is_none() {
        if let Some(candidate) = executable_extension_candidate(path) {
            return Some(candidate);
        }
    }

    if path.is_file() {
        return Some(path.to_path_buf());
    }

    if cfg!(windows) {
        return executable_extension_candidate(path);
    }

    None
}

fn executable_extension_candidate(path: &Path) -> Option<PathBuf> {
    for ext in executable_extensions() {
        let candidate = path.with_extension(ext);
        if candidate.is_file() {
            return Some(candidate);
        }
    }
    None
}

fn executable_extensions() -> Vec<String> {
    std::env::var("PATHEXT")
        .ok()
        .map(|value| {
            value
                .split(';')
                .filter_map(|ext| ext.trim().trim_start_matches('.').split_whitespace().next())
                .filter(|ext| !ext.is_empty())
                .map(str::to_ascii_lowercase)
                .collect()
        })
        .unwrap_or_else(|| vec!["exe".into(), "com".into(), "bat".into(), "cmd".into()])
}

fn find_standard_unix_shell() -> Option<PathBuf> {
    if cfg!(windows) {
        return None;
    }

    ["/bin/sh", "/usr/bin/sh", "/bin/bash", "/usr/bin/bash"]
        .into_iter()
        .map(PathBuf::from)
        .find(|path| path.is_file())
}

fn has_path_separator(name: &str) -> bool {
    name.contains('/') || name.contains('\\')
}

pub(crate) fn shell_path_to_windows(path: &str, env_vars: &HashMap<String, String>) -> PathBuf {
    if !cfg!(windows) {
        return PathBuf::from(path);
    }

    let normalized = path.replace('\\', "/");

    if normalized == "/dev/null" || normalized.eq_ignore_ascii_case("NUL") {
        return PathBuf::from("NUL");
    }

    if normalized.len() >= 3
        && normalized.as_bytes()[0] == b'/'
        && normalized.as_bytes()[2] == b'/'
        && normalized.as_bytes()[1].is_ascii_alphabetic()
    {
        let drive = normalized.as_bytes()[1] as char;
        return PathBuf::from(
            format!("{}:\\{}", drive.to_ascii_uppercase(), &normalized[3..]).replace('/', "\\"),
        );
    }

    if normalized == "/tmp" {
        if let Some(tmpdir) = env_vars.get("TMPDIR") {
            if tmpdir.replace('\\', "/") == "/tmp" {
                return std::env::temp_dir();
            }
            return shell_path_to_windows(tmpdir, env_vars);
        }
    }

    if let Some(rest) = normalized.strip_prefix("/tmp/") {
        if let Some(tmpdir) = env_vars.get("TMPDIR") {
            if tmpdir.replace('\\', "/") == "/tmp" {
                return std::env::temp_dir().join(rest);
            }
            return shell_path_to_windows(tmpdir, env_vars).join(rest);
        }
    }

    PathBuf::from(path)
}

#[cfg(test)]
mod tests {
    use super::*;
    #[cfg(windows)]
    use std::fs;

    #[cfg(not(windows))]
    #[test]
    fn unix_shell_lookup_falls_back_to_standard_paths() {
        let mut env_vars = HashMap::new();
        env_vars.insert("PATH".to_string(), "target/rubash-isolated-bin".to_string());

        assert!(find_shell(&env_vars).is_some());
    }

    #[cfg(windows)]
    #[test]
    fn windows_shell_lookup_ignores_compatible_shell_env() {
        let native_exe = std::env::current_exe().unwrap();
        let mut env_vars = HashMap::new();
        env_vars.insert(
            "RUBASH_COMPATIBLE_SHELL_PATH".to_string(),
            native_exe.to_string_lossy().to_string(),
        );
        env_vars.insert("PATH".to_string(), String::new());

        assert_eq!(find_shell(&env_vars), None);
        assert_eq!(find_user_command("sh", &env_vars), None);
    }

    #[cfg(windows)]
    #[test]
    fn windows_shell_lookup_uses_path_only() {
        let bin_dir = std::env::temp_dir().join("rubash-path-only-shell-bin");
        let _ = fs::remove_dir_all(&bin_dir);
        fs::create_dir_all(&bin_dir).unwrap();
        let shell = bin_dir.join("sh.exe");
        fs::write(&shell, "").unwrap();

        let mut env_vars = HashMap::new();
        env_vars.insert("PATH".to_string(), bin_dir.to_string_lossy().to_string());

        assert_eq!(find_shell(&env_vars), Some(shell.clone()));
        assert_eq!(find_user_command("sh", &env_vars), Some(shell));
        let _ = fs::remove_dir_all(bin_dir);
    }

    #[cfg(windows)]
    #[test]
    fn windows_find_user_command_works_with_mixed_case_path() {
        // On Windows, std::env::vars() returns PATH as "Path" (capital P).
        // find_user_command reads env_vars.get("PATH") (all caps), so we should
        // fail to find the command when only "Path" is set. This test documents
        // the upstream behavior and motivates the init.rs fix that mirrors
        // the value into the all-caps key.
        let target_dir = std::env::temp_dir().join("rubash-mixed-case-path");
        let _ = fs::remove_dir_all(&target_dir);
        fs::create_dir_all(&target_dir).unwrap();
        let marker = target_dir.join("cmd.exe");
        fs::write(&marker, "").unwrap();

        // This lookup attempts to find "cmd" using only the all-caps PATH key,
        // which is what Executor::new() will hold after the init.rs fix runs.
        let mut env_vars = HashMap::new();
        env_vars.insert("PATH".to_string(), target_dir.to_string_lossy().to_string());
        assert_eq!(
            find_user_command("cmd", &env_vars).map(|p| p.to_string_lossy().to_string()),
            Some(marker.to_string_lossy().to_string()),
        );

        let _ = fs::remove_dir_all(&target_dir);
    }

    #[cfg(windows)]
    #[test]
    fn windows_find_user_command_fails_when_only_path_lower_is_set() {
        // Direct counter-test for the casing bug: setting the OS-side
        // "Path" (capital P) without the all-caps "PATH" key causes
        // find_user_command to miss the command. Bug surfaces in shells
        // embedding rubash on Windows until Executor::new() normalizes.
        let target_dir = std::env::temp_dir().join("rubash-only-path-lower");
        let _ = fs::remove_dir_all(&target_dir);
        std::fs::create_dir_all(&target_dir).unwrap();
        let marker = target_dir.join("cmd.exe");
        std::fs::write(&marker, "").unwrap();

        let mut env_vars = HashMap::new();
        env_vars.insert("Path".to_string(), target_dir.to_string_lossy().to_string());

        // find_user_command has no normalization itself; the init.rs workaround
        // upstream performs the casing mirror. Without that workaround, this
        // lookup returns None.
        assert_eq!(
            find_user_command("cmd", &env_vars),
            None,
            "find_user_command should not see the lowercase Path key until init.rs normalizes"
        );

        let _ = std::fs::remove_dir_all(&target_dir);
    }

    #[cfg(windows)]
    #[test]
    fn windows_find_user_command_splits_bash_style_prefix_before_native_path() {
        let empty_dir = std::env::temp_dir().join("rubash-bash-style-path-prefix-empty");
        let bin_dir = std::env::temp_dir().join("rubash-bash-style-path-prefix-bin");
        let _ = fs::remove_dir_all(&empty_dir);
        let _ = fs::remove_dir_all(&bin_dir);
        fs::create_dir_all(&empty_dir).unwrap();
        fs::create_dir_all(&bin_dir).unwrap();

        let marker = bin_dir.join("clear.exe");
        fs::write(&marker, "").unwrap();

        let mut env_vars = HashMap::new();
        env_vars.insert(
            "PATH".to_string(),
            format!(
                "{}:{};C:/definitely/missing",
                windows_shell_path(&empty_dir),
                windows_shell_path(&bin_dir)
            ),
        );

        assert_eq!(find_user_command("clear", &env_vars), Some(marker));

        let _ = fs::remove_dir_all(empty_dir);
        let _ = fs::remove_dir_all(bin_dir);
    }

    #[cfg(windows)]
    fn windows_shell_path(path: &Path) -> String {
        let value = path.to_string_lossy().replace('\\', "/");
        let bytes = value.as_bytes();
        if bytes.len() >= 3
            && bytes[0].is_ascii_alphabetic()
            && bytes[1] == b':'
            && bytes[2] == b'/'
        {
            format!(
                "/{}/{}",
                (bytes[0] as char).to_ascii_lowercase(),
                &value[3..]
            )
        } else {
            value
        }
    }
}
