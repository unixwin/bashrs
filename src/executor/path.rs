//! path module.
//!
//! GNU Bash source ownership:
// - findcmd.c
// - findcmd.h

use std::collections::{HashMap, HashSet};
use std::fs;
use std::io;
use std::path::{Path, PathBuf};
use std::process::Command;

use super::support_names::split_shell_path;

pub(crate) fn shell_path_entries(path: &str) -> Vec<String> {
    split_shell_path(path)
}

/// A directory entry in the shell namespace.
///
/// Logical command directories can contain both files from the configured
/// shell root and files supplied by the selected WinuxCmd provider. Callers
/// must use this view instead of enumerating the backing directory directly.
#[derive(Debug, Clone)]
pub(crate) struct ShellDirectoryEntry {
    pub(crate) name: String,
    pub(crate) path: PathBuf,
    pub(crate) is_dir: bool,
    pub(crate) is_file: bool,
}

/// Enumerate a shell-visible directory, including the WinuxCmd command view
/// for `/bin`, `/usr/bin`, and `/usr/local/bin` on Windows.
pub(crate) fn shell_directory_entries(
    path: &str,
    env_vars: &HashMap<String, String>,
) -> io::Result<Vec<ShellDirectoryEntry>> {
    let physical_dir = shell_path_to_windows(path, env_vars);
    let mut entries = Vec::new();
    let mut names = HashSet::new();
    let mut physical_error = None;

    match fs::read_dir(&physical_dir) {
        Ok(directory) => {
            for entry in directory {
                let entry = entry?;
                let name = entry.file_name().to_string_lossy().into_owned();
                let key = name.to_ascii_lowercase();
                let file_type = entry.file_type()?;
                names.insert(key);
                entries.push(ShellDirectoryEntry {
                    name,
                    path: entry.path(),
                    is_dir: file_type.is_dir(),
                    is_file: file_type.is_file(),
                });
            }
        }
        Err(error) => physical_error = Some(error),
    }

    #[cfg(windows)]
    if let Some(provider_dir) = winuxcmd_provider_directory_for_logical(path, env_vars) {
        if let Ok(directory) = fs::read_dir(provider_dir) {
            for entry in directory.flatten() {
                let name = entry.file_name().to_string_lossy().into_owned();
                let key = name.to_ascii_lowercase();
                if names.contains(&key) || !is_provider_command_file(&entry.path()) {
                    continue;
                }
                let Ok(file_type) = entry.file_type() else {
                    continue;
                };
                names.insert(key);
                entries.push(ShellDirectoryEntry {
                    name,
                    path: entry.path(),
                    is_dir: file_type.is_dir(),
                    is_file: file_type.is_file(),
                });
            }
        }
    }

    if entries.is_empty() {
        if let Some(error) = physical_error {
            return Err(error);
        }
    }

    Ok(entries)
}

pub fn find_user_command(name: &str, env_vars: &HashMap<String, String>) -> Option<PathBuf> {
    if name.is_empty() {
        return None;
    }

    if has_path_separator(name) {
        let candidate = shell_path_to_windows(name, env_vars);
        if let Some(found) = executable_candidate(&candidate, env_vars) {
            return Some(found);
        }
        #[cfg(windows)]
        if let Some(found) = find_winuxcmd_provider_command(name, env_vars) {
            return Some(found);
        }
        #[cfg(windows)]
        if let Some(found) = find_winuxcmd_absolute_command(name, env_vars) {
            return Some(found);
        }
        return None;
    }

    for dir in split_shell_path(env_vars.get("PATH").map(String::as_str).unwrap_or_default()) {
        let candidate = shell_path_to_windows(&dir, env_vars).join(name);
        if let Some(found) = executable_candidate(&candidate, env_vars) {
            return Some(found);
        }
        #[cfg(windows)]
        if let Some(found) = find_winuxcmd_path_entry_command(&dir, name, env_vars) {
            return Some(found);
        }
    }

    // A workspace may expose WinuxCmd as one dispatcher executable instead of
    // one wrapper per command.  Ask the dispatcher whether it owns the name
    // before returning it; unknown names must retain Bash's 127 behavior.
    #[cfg(windows)]
    if let Some(dispatcher) = find_winuxcmd_dispatcher(env_vars) {
        if winuxcmd_has_command(&dispatcher, name, env_vars) {
            return Some(dispatcher);
        }
    }

    None
}

pub fn standard_path(_env_vars: &HashMap<String, String>) -> String {
    if cfg!(windows) {
        if configured_shell_root(_env_vars).is_some() {
            return "/usr/local/bin:/usr/bin:/bin".to_string();
        }
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
    external_command_for_named_program(program, None, args, env_vars)
}

pub fn external_command_for_named_program(
    program: &Path,
    command_name: Option<&str>,
    args: &[String],
    env_vars: &HashMap<String, String>,
) -> (Command, bool) {
    let native_args = args
        .iter()
        .map(|arg| external_argument_path(arg, env_vars))
        .collect::<Vec<_>>();

    if is_windows_powershell_script(program) {
        let program = cmd_compatible_windows_path(program);
        let mut command = Command::new(windows_powershell_processor(env_vars));
        command
            .arg("-NoProfile")
            .arg("-ExecutionPolicy")
            .arg("Bypass")
            .arg("-File")
            .arg(program);
        command.args(&native_args);
        return (command, false);
    }

    if is_windows_batch_file(program) {
        let program = cmd_compatible_windows_path(program);
        let mut command = Command::new(windows_command_processor());
        command.arg("/D").arg("/C").arg(program);
        command.args(&native_args);
        return (command, false);
    }

    if should_run_with_shell(program) {
        if let Some(shell) = find_shell(env_vars) {
            let mut command = Command::new(shell);
            command.arg(program);
            command.args(&native_args);
            return (command, true);
        }
        if let Some(shell) = current_shell_processor() {
            let mut command = Command::new(shell);
            command.arg(program);
            command.args(&native_args);
            return (command, true);
        }
    }

    let mut command = Command::new(program);
    if is_winuxcmd_dispatcher(program) {
        if let Some(command_name) = command_name {
            command.arg(dispatcher_command_name(command_name));
        }
    }
    command.args(&native_args);
    (command, false)
}

fn is_winuxcmd_dispatcher(path: &Path) -> bool {
    cfg!(windows)
        && path
            .file_stem()
            .and_then(|stem| stem.to_str())
            .is_some_and(|stem| stem.eq_ignore_ascii_case("winuxcmd"))
}

#[cfg(windows)]
fn find_winuxcmd_dispatcher(env_vars: &HashMap<String, String>) -> Option<PathBuf> {
    for name in ["WINUXCMD", "WINUXCMD_PATH"] {
        if let Some(value) = env_vars.get(name) {
            let candidate = shell_path_to_windows(value, env_vars);
            if let Some(found) = executable_candidate(&candidate, env_vars) {
                return Some(found);
            }
        }
    }

    // Winuxsh owns WinuxCmd selection. Rubash must not guess a dispatcher from
    // PATH because a process can contain command links from a different
    // WinuxCmd installation. Embedders may still provide an explicit path via
    // WINUXCMD_PATH/WINUXCMD or `Executor::set_winuxcmd_path`.
    None
}

#[cfg(windows)]
fn winuxcmd_has_command(dispatcher: &Path, name: &str, env_vars: &HashMap<String, String>) -> bool {
    let mut command = Command::new(dispatcher);
    command.arg("help").arg(name);
    for key in ["SystemRoot", "WINDIR", "ComSpec"] {
        if let Some(value) = env_vars.get(key) {
            command.env(key, value);
        }
    }
    command.output().is_ok_and(|output| output.status.success())
}

#[cfg(windows)]
fn find_winuxcmd_absolute_command(
    name: &str,
    env_vars: &HashMap<String, String>,
) -> Option<PathBuf> {
    let command_name = logical_bin_command_name(name)?;
    let dispatcher = find_winuxcmd_dispatcher(env_vars)?;
    winuxcmd_has_command(&dispatcher, &command_name, env_vars).then_some(dispatcher)
}

#[cfg(windows)]
fn find_winuxcmd_provider_command(
    name: &str,
    env_vars: &HashMap<String, String>,
) -> Option<PathBuf> {
    let (directory, command_name) = logical_bin_path_parts(name)?;
    find_winuxcmd_provider(&command_name, Some(&directory), env_vars)
}

#[cfg(windows)]
fn find_winuxcmd_path_entry_command(
    directory: &str,
    name: &str,
    env_vars: &HashMap<String, String>,
) -> Option<PathBuf> {
    let normalized = directory.replace('\\', "/");
    let normalized = normalized.trim_end_matches('/');
    if !matches!(normalized, "/bin" | "/usr/bin" | "/usr/local/bin") {
        return None;
    }
    find_winuxcmd_provider(name, Some(normalized), env_vars)
}

#[cfg(windows)]
fn find_winuxcmd_provider(
    name: &str,
    logical_directory: Option<&str>,
    env_vars: &HashMap<String, String>,
) -> Option<PathBuf> {
    let home = winuxcmd_provider_home(env_vars)?;
    let file_name = if name.to_ascii_lowercase().ends_with(".exe") {
        name.to_string()
    } else {
        format!("{name}.exe")
    };
    let mut relative_directories = Vec::new();
    if let Some(directory) = logical_directory {
        let directory = directory.trim_matches('/');
        if !directory.is_empty() {
            relative_directories.push(directory.to_string());
        }
    }
    relative_directories.push(String::new());
    for directory in ["bin", "usr/bin", "usr/local/bin"] {
        if !relative_directories.iter().any(|entry| entry == directory) {
            relative_directories.push(directory.to_string());
        }
    }

    relative_directories.into_iter().find_map(|directory| {
        let candidate = if directory.is_empty() {
            home.join(&file_name)
        } else {
            home.join(directory).join(&file_name)
        };
        executable_candidate(&candidate, env_vars)
    })
}

#[cfg(windows)]
fn winuxcmd_provider_home(env_vars: &HashMap<String, String>) -> Option<PathBuf> {
    let home = env_vars
        .get("WINUXCMD_HOME")
        .or_else(|| env_vars.get("WINUXCMD_PATH"))
        .map(|value| shell_path_to_windows(value, env_vars))?;
    if home.is_file() {
        home.parent().map(Path::to_path_buf)
    } else {
        Some(home)
    }
}

#[cfg(windows)]
fn winuxcmd_provider_directory_for_logical(
    logical_path: &str,
    env_vars: &HashMap<String, String>,
) -> Option<PathBuf> {
    let logical_directory = logical_bin_directory(logical_path)?;
    let home = winuxcmd_provider_home(env_vars)?;
    let nested = home.join(logical_directory.trim_start_matches('/'));
    if nested.is_dir() {
        Some(nested)
    } else {
        // The installed WinuxCmd/WPM layout is currently flat. The provider
        // root is used as a command-only view; `.wpm` is never emitted because
        // directory enumeration accepts executable files only.
        Some(home)
    }
}

#[cfg(windows)]
fn is_provider_command_file(path: &Path) -> bool {
    if !path.is_file() {
        return false;
    }
    path.extension().is_none_or(|extension| {
        matches!(
            extension.to_string_lossy().to_ascii_lowercase().as_str(),
            "exe" | "com" | "bat" | "cmd" | "ps1"
        )
    })
}

fn logical_bin_directory(path: &str) -> Option<&'static str> {
    let normalized = path.replace('\\', "/");
    match normalized.trim_end_matches('/') {
        "/bin" => Some("/bin"),
        "/usr/bin" => Some("/usr/bin"),
        "/usr/local/bin" => Some("/usr/local/bin"),
        _ => None,
    }
}

/// Return the native directories a Windows child should receive for one shell
/// PATH entry. A logical command directory needs both its writable backing
/// directory and the selected provider directory; native child processes do
/// not understand the logical overlay themselves.
pub(crate) fn shell_path_process_entries(
    path: &str,
    env_vars: &HashMap<String, String>,
) -> Vec<PathBuf> {
    let physical = shell_path_to_windows(path, env_vars);
    #[cfg(windows)]
    if logical_bin_directory(path).is_some() {
        let mut entries = vec![physical];
        if let Some(provider) = winuxcmd_provider_directory_for_logical(path, env_vars) {
            if !entries.iter().any(|entry| entry == &provider) {
                entries.push(provider);
            }
        }
        return entries;
    }
    vec![physical]
}

/// Materialize a shell PATH for a native child process.
///
/// The shell may keep logical command directories in `PATH`, but a Windows
/// child needs the root-backed directory and the selected provider directory
/// as separate native entries. This is the PATH equivalent of the filesystem
/// overlay used by command lookup.
pub(crate) fn shell_path_to_process(path: &str, env_vars: &HashMap<String, String>) -> String {
    let separator = if cfg!(windows) { ';' } else { ':' };
    shell_path_entries(path)
        .into_iter()
        .flat_map(|entry| shell_path_process_entries(&entry, env_vars))
        .map(|entry| entry.to_string_lossy().replace('/', "\\"))
        .collect::<Vec<_>>()
        .join(&separator.to_string())
}

#[cfg(windows)]
fn logical_bin_path_parts(name: &str) -> Option<(String, String)> {
    let normalized = name.replace('\\', "/");
    let directory = ["/bin/", "/usr/bin/", "/usr/local/bin/"]
        .into_iter()
        .find(|prefix| normalized.starts_with(prefix))?;
    let rest = normalized.strip_prefix(directory)?;
    if rest.is_empty() || rest.contains('/') || rest.contains('\\') {
        return None;
    }
    Some((
        directory.trim_end_matches('/').to_string(),
        rest.to_string(),
    ))
}

#[cfg(windows)]
fn logical_bin_command_name(name: &str) -> Option<String> {
    let normalized = name.replace('\\', "/");
    let rest = normalized
        .strip_prefix("/bin/")
        .or_else(|| normalized.strip_prefix("/usr/bin/"))
        .or_else(|| normalized.strip_prefix("/usr/local/bin/"))?;
    if rest.is_empty() || rest.contains('/') || rest.contains('\\') {
        return None;
    }
    Some(rest.to_string())
}

fn external_argument_path(arg: &str, env_vars: &HashMap<String, String>) -> String {
    if cfg!(windows) {
        let normalized = arg.replace('\\', "/");
        let drive_path = normalized.len() >= 3
            && normalized.as_bytes()[0] == b'/'
            && normalized.as_bytes()[2] == b'/'
            && normalized.as_bytes()[1].is_ascii_alphabetic();
        if drive_path {
            let drive = normalized.as_bytes()[1].to_ascii_lowercase();
            let translated = shell_path_to_windows(arg, env_vars);
            // `/c/...` is Rubash's explicit POSIX display-path spelling and
            // must be translated even before the target is created. Other
            // `/X/...` arguments are ambiguous (regexes, git pathspecs,
            // sed/awk fragments); convert those only when they resolve to a
            // real filesystem path.
            if drive != b'c' && normalized.len() <= 3 {
                return arg.to_string();
            }
            if drive != b'c' && !translated.exists() {
                return arg.to_string();
            }
            return translated.to_string_lossy().into_owned();
        }
        shell_path_to_windows_for_lookup(arg, env_vars)
            .to_string_lossy()
            .into_owned()
    } else {
        arg.to_string()
    }
}

fn dispatcher_command_name(command_name: &str) -> String {
    if let Some(name) = logical_bin_command_name_any_platform(command_name) {
        return name;
    }
    command_name
        .replace('\\', "/")
        .rsplit('/')
        .next()
        .unwrap_or(command_name)
        .to_string()
}

fn logical_bin_command_name_any_platform(name: &str) -> Option<String> {
    let normalized = name.replace('\\', "/");
    let rest = normalized
        .strip_prefix("/bin/")
        .or_else(|| normalized.strip_prefix("/usr/bin/"))
        .or_else(|| normalized.strip_prefix("/usr/local/bin/"))?;
    if rest.is_empty() || rest.contains('/') || rest.contains('\\') {
        return None;
    }
    Some(rest.to_string())
}

fn current_shell_processor() -> Option<PathBuf> {
    std::env::current_exe().ok()
}

fn is_windows_powershell_script(path: &Path) -> bool {
    cfg!(windows)
        && path
            .extension()
            .and_then(|ext| ext.to_str())
            .is_some_and(|ext| ext.eq_ignore_ascii_case("ps1"))
}

fn is_windows_batch_file(path: &Path) -> bool {
    cfg!(windows)
        && path
            .extension()
            .and_then(|ext| ext.to_str())
            .is_some_and(|ext| matches!(ext.to_ascii_lowercase().as_str(), "bat" | "cmd"))
}

fn windows_powershell_processor(env_vars: &HashMap<String, String>) -> PathBuf {
    find_user_command("pwsh", env_vars)
        .or_else(|| find_user_command("powershell", env_vars))
        .or_else(|| {
            let system_root = env_vars
                .get("SystemRoot")
                .or_else(|| env_vars.get("WINDIR"))
                .cloned()
                .or_else(|| std::env::var("SystemRoot").ok())
                .or_else(|| std::env::var("WINDIR").ok())?;
            let path = PathBuf::from(system_root)
                .join("System32")
                .join("WindowsPowerShell")
                .join("v1.0")
                .join("powershell.exe");
            path.is_file().then_some(path)
        })
        .unwrap_or_else(|| PathBuf::from("pwsh"))
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

fn executable_candidate(path: &Path, env_vars: &HashMap<String, String>) -> Option<PathBuf> {
    if cfg!(windows) && path.extension().is_none() {
        if let Some(candidate) = executable_extension_candidate(path, env_vars) {
            return Some(candidate);
        }
    }

    if path.is_file() {
        return Some(path.to_path_buf());
    }

    if cfg!(windows) {
        return executable_extension_candidate(path, env_vars);
    }

    None
}

fn executable_extension_candidate(
    path: &Path,
    env_vars: &HashMap<String, String>,
) -> Option<PathBuf> {
    for ext in executable_extensions(env_vars) {
        let candidate = path.with_extension(ext);
        if candidate.is_file() {
            return Some(candidate);
        }
    }
    None
}

fn executable_extensions(env_vars: &HashMap<String, String>) -> Vec<String> {
    let mut exts = env_vars
        .get("PATHEXT")
        .cloned()
        .or_else(|| std::env::var("PATHEXT").ok())
        .map(|value| {
            value
                .split(';')
                .filter_map(|ext| ext.trim().trim_start_matches('.').split_whitespace().next())
                .filter(|ext| !ext.is_empty())
                .map(str::to_ascii_lowercase)
                .collect()
        })
        .unwrap_or_else(|| vec!["exe".into(), "com".into(), "bat".into(), "cmd".into()]);

    for ext in ["exe", "com", "bat", "cmd", "ps1"] {
        if !exts
            .iter()
            .any(|candidate| candidate.eq_ignore_ascii_case(ext))
        {
            exts.push(ext.into());
        }
    }
    exts
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
    let normalized = path.replace('\\', "/");
    let shell_root = configured_shell_root(env_vars);

    if cfg!(windows) && (normalized == "/dev/null" || normalized.eq_ignore_ascii_case("NUL")) {
        return PathBuf::from("NUL");
    }

    // `/dev` is a capability namespace. Only `/dev/null` is currently mapped
    // on Windows; do not let unsupported fd/tty spellings become ordinary
    // files below the logical root.
    if cfg!(windows) && (normalized == "/dev" || normalized.starts_with("/dev/")) {
        return PathBuf::from(r"\\.\WINUXSH_UNSUPPORTED_DEVICE");
    }

    if cfg!(windows)
        && normalized.len() >= 3
        && normalized.as_bytes()[0] == b'/'
        && normalized.as_bytes()[2] == b'/'
        && normalized.as_bytes()[1].is_ascii_alphabetic()
    {
        let drive = normalized.as_bytes()[1] as char;
        return PathBuf::from(
            format!("{}:\\{}", drive.to_ascii_uppercase(), &normalized[3..]).replace('/', "\\"),
        );
    }

    if cfg!(windows) && shell_root.is_none() && normalized == "/tmp" {
        if let Some(tmpdir) = env_vars.get("TMPDIR") {
            if tmpdir.replace('\\', "/") == "/tmp" {
                return std::env::temp_dir();
            }
            return shell_path_to_windows(tmpdir, env_vars);
        }
    }

    if cfg!(windows) && shell_root.is_none() {
        if let Some(rest) = normalized.strip_prefix("/tmp/") {
            if let Some(tmpdir) = env_vars.get("TMPDIR") {
                if tmpdir.replace('\\', "/") == "/tmp" {
                    return std::env::temp_dir().join(rest);
                }
                return shell_path_to_windows(tmpdir, env_vars).join(rest);
            }
        }
    }

    if let Some(root) = shell_root {
        if let Some(mapped) = map_logical_path(&normalized, &root) {
            return mapped;
        }
    }

    PathBuf::from(path)
}

pub(crate) fn shell_path_to_windows_for_lookup(
    path: &str,
    env_vars: &HashMap<String, String>,
) -> PathBuf {
    let mapped = shell_path_to_windows(path, env_vars);
    if mapped.exists() {
        return mapped;
    }

    #[cfg(windows)]
    if let Some(found) = find_winuxcmd_provider_command(path, env_vars) {
        return found;
    }

    mapped
}

pub(crate) fn resolve_shell_path_from_env(
    path: &str,
    env_vars: &HashMap<String, String>,
) -> PathBuf {
    shell_path_to_windows(path, env_vars)
}

pub(crate) fn is_shell_null_device(path: &str) -> bool {
    let normalized = path.replace('\\', "/");
    normalized == "/dev/null" || (cfg!(windows) && normalized.eq_ignore_ascii_case("NUL"))
}

fn configured_shell_root(env_vars: &HashMap<String, String>) -> Option<PathBuf> {
    let value = ["__RUBASH_SHELL_ROOT", "WINUXSH_ROOT", "RUBASH_ROOT"]
        .into_iter()
        .find_map(|name| env_vars.get(name))
        .filter(|value| !value.is_empty())?;

    let normalized = value.replace('\\', "/");
    if cfg!(windows)
        && normalized.len() >= 3
        && normalized.as_bytes()[0] == b'/'
        && normalized.as_bytes()[2] == b'/'
        && normalized.as_bytes()[1].is_ascii_alphabetic()
    {
        let drive = normalized.as_bytes()[1] as char;
        return Some(PathBuf::from(
            format!("{}:\\{}", drive.to_ascii_uppercase(), &normalized[3..]).replace('/', "\\"),
        ));
    }

    Some(PathBuf::from(value))
}

pub(crate) fn shell_root_configured(env_vars: &HashMap<String, String>) -> bool {
    configured_shell_root(env_vars).is_some()
}

fn map_logical_path(normalized: &str, root: &Path) -> Option<PathBuf> {
    if !normalized.starts_with('/') || normalized.starts_with("//") {
        return None;
    }
    if normalized.len() >= 3
        && normalized.as_bytes()[0] == b'/'
        && normalized.as_bytes()[2] == b'/'
        && normalized.as_bytes()[1].is_ascii_alphabetic()
    {
        return None;
    }

    let mut components = Vec::new();
    for component in normalized[1..].split('/') {
        match component {
            "" | "." => {}
            ".." => {
                components.pop();
            }
            component => components.push(component),
        }
    }

    let mut mapped = root.to_path_buf();
    for component in components {
        mapped.push(component);
    }
    Some(mapped)
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
    #[test]
    fn windows_find_user_command_maps_logical_usr_bin_from_shell_root() {
        let root = std::env::temp_dir().join("rubash-logical-root-absolute-command");
        let _ = fs::remove_dir_all(&root);
        fs::create_dir_all(root.join("usr").join("bin")).unwrap();
        let command = root.join("usr").join("bin").join("tool.exe");
        fs::write(&command, "").unwrap();

        let mut env_vars = HashMap::new();
        env_vars.insert(
            "RUBASH_ROOT".to_string(),
            root.to_string_lossy().to_string(),
        );

        assert_eq!(find_user_command("/usr/bin/tool", &env_vars), Some(command));
        let _ = fs::remove_dir_all(root);
    }

    #[cfg(windows)]
    #[test]
    fn windows_shell_root_maps_root_and_clamps_parent_components() {
        let root = std::env::temp_dir().join("rubash-logical-root-paths");
        let mut env_vars = HashMap::new();
        env_vars.insert(
            "RUBASH_ROOT".to_string(),
            root.to_string_lossy().to_string(),
        );
        env_vars.insert("TMPDIR".to_string(), r"C:\Windows\Temp".to_string());

        assert_eq!(shell_path_to_windows("/", &env_vars), root);
        assert_eq!(
            shell_path_to_windows("/bin/../etc/config", &env_vars),
            root.join("etc").join("config")
        );
        assert_eq!(
            shell_path_to_windows("/tmp/cache", &env_vars),
            root.join("tmp").join("cache")
        );
        assert_eq!(
            shell_path_to_windows("/c/Users/example", &env_vars),
            PathBuf::from(r"C:\Users\example")
        );
        assert_eq!(
            shell_path_to_windows("C:/Users/example", &env_vars),
            PathBuf::from(r"C:/Users/example")
        );
        assert_eq!(
            shell_path_to_windows("/dev/null", &env_vars),
            PathBuf::from("NUL")
        );
        assert_eq!(
            shell_path_to_windows("/dev/fd/1", &env_vars),
            PathBuf::from(r"\\.\WINUXSH_UNSUPPORTED_DEVICE")
        );
    }

    #[cfg(windows)]
    #[test]
    fn windows_shell_root_selects_logical_standard_path() {
        let mut env_vars = HashMap::new();
        env_vars.insert(
            "WINUXSH_ROOT".to_string(),
            std::env::temp_dir().to_string_lossy().to_string(),
        );

        assert_eq!(
            standard_path(&env_vars),
            "/usr/local/bin:/usr/bin:/bin".to_string()
        );
    }

    #[cfg(windows)]
    #[test]
    fn windows_dispatcher_requires_explicit_session_path() {
        let dir = std::env::temp_dir().join("rubash-explicit-winuxcmd-dispatcher");
        let _ = fs::remove_dir_all(&dir);
        fs::create_dir_all(&dir).unwrap();
        let dispatcher = dir.join("winuxcmd.exe");
        fs::write(&dispatcher, b"").unwrap();

        let mut env_vars = HashMap::new();
        env_vars.insert("PATH".to_string(), dir.to_string_lossy().to_string());
        assert_eq!(find_winuxcmd_dispatcher(&env_vars), None);

        env_vars.insert(
            "WINUXCMD_PATH".to_string(),
            dispatcher.to_string_lossy().to_string(),
        );
        assert_eq!(find_winuxcmd_dispatcher(&env_vars), Some(dispatcher));
        let _ = fs::remove_dir_all(dir);
    }

    #[cfg(windows)]
    #[test]
    fn windows_logical_bin_paths_use_selected_winuxcmd_links_without_copying() {
        let root = std::env::temp_dir().join("rubash-logical-bin-provider");
        let _ = fs::remove_dir_all(&root);
        let shell_root = root.join("root");
        let winuxcmd_home = root.join("winuxcmd");
        fs::create_dir_all(shell_root.join("usr").join("bin")).unwrap();
        fs::create_dir_all(&winuxcmd_home).unwrap();
        fs::create_dir_all(winuxcmd_home.join("usr").join("bin")).unwrap();

        let dispatcher = winuxcmd_home.join("winuxcmd.exe");
        let link = winuxcmd_home.join("ls.exe");
        let nested_provider = winuxcmd_home.join("usr").join("bin").join("awk.exe");
        fs::write(&dispatcher, b"").unwrap();
        fs::write(&link, b"").unwrap();
        fs::write(&nested_provider, b"").unwrap();

        let mut env_vars = HashMap::new();
        env_vars.insert(
            "WINUXSH_ROOT".to_string(),
            shell_root.to_string_lossy().to_string(),
        );
        env_vars.insert(
            "WINUXCMD_PATH".to_string(),
            dispatcher.to_string_lossy().to_string(),
        );
        env_vars.insert(
            "WINUXCMD_HOME".to_string(),
            winuxcmd_home.to_string_lossy().to_string(),
        );

        assert_eq!(
            find_user_command("/usr/bin/ls", &env_vars),
            Some(link.clone())
        );
        assert_eq!(shell_path_to_windows_for_lookup("/bin/ls", &env_vars), link);
        assert_eq!(
            shell_path_to_windows_for_lookup("/usr/bin/awk", &env_vars),
            nested_provider
        );
        assert!(!shell_root.join("usr").join("bin").join("ls.exe").exists());

        let _ = fs::remove_dir_all(root);
    }

    #[cfg(windows)]
    #[test]
    fn windows_logical_bin_directory_view_excludes_wpm_state() {
        let root = std::env::temp_dir().join("rubash-logical-bin-directory-view");
        let _ = fs::remove_dir_all(&root);
        let shell_root = root.join("root");
        let winuxcmd_home = root.join("winuxcmd");
        fs::create_dir_all(shell_root.join("usr").join("bin")).unwrap();
        fs::create_dir_all(winuxcmd_home.join(".wpm").join("cache")).unwrap();
        fs::write(shell_root.join("usr").join("bin").join("local.exe"), b"").unwrap();
        fs::write(winuxcmd_home.join("ls.exe"), b"").unwrap();
        fs::write(winuxcmd_home.join("wpm.exe"), b"").unwrap();
        fs::write(winuxcmd_home.join(".wpm").join("cache").join("jq.exe"), b"").unwrap();

        let mut env_vars = HashMap::new();
        env_vars.insert(
            "WINUXSH_ROOT".to_string(),
            shell_root.to_string_lossy().to_string(),
        );
        env_vars.insert(
            "WINUXCMD_HOME".to_string(),
            winuxcmd_home.to_string_lossy().to_string(),
        );

        let entries = shell_directory_entries("/usr/bin", &env_vars).unwrap();
        let names = entries
            .into_iter()
            .map(|entry| entry.name)
            .collect::<HashSet<_>>();
        assert!(names.contains("local.exe"));
        assert!(names.contains("ls.exe"));
        assert!(names.contains("wpm.exe"));
        assert!(!names.contains(".wpm"));
        assert!(!names.contains("jq.exe"));

        let _ = fs::remove_dir_all(root);
    }

    #[cfg(windows)]
    #[test]
    fn windows_logical_path_process_entries_include_root_and_provider() {
        let root = std::env::temp_dir().join("rubash-logical-path-process-entries");
        let _ = fs::remove_dir_all(&root);
        let shell_root = root.join("root");
        let winuxcmd_home = root.join("winuxcmd");
        fs::create_dir_all(shell_root.join("usr").join("bin")).unwrap();
        fs::create_dir_all(&winuxcmd_home).unwrap();

        let mut env_vars = HashMap::new();
        env_vars.insert(
            "WINUXSH_ROOT".to_string(),
            shell_root.to_string_lossy().to_string(),
        );
        env_vars.insert(
            "WINUXCMD_HOME".to_string(),
            winuxcmd_home.to_string_lossy().to_string(),
        );

        let entries = shell_path_process_entries("/usr/bin", &env_vars);
        assert_eq!(
            entries,
            vec![shell_root.join("usr").join("bin"), winuxcmd_home]
        );

        let _ = fs::remove_dir_all(root);
    }

    #[test]
    fn dispatcher_command_name_strips_logical_bin_prefixes() {
        assert_eq!(dispatcher_command_name("head"), "head");
        assert_eq!(dispatcher_command_name("/bin/cat"), "cat");
        assert_eq!(dispatcher_command_name("/usr/bin/cat"), "cat");
        assert_eq!(dispatcher_command_name("/usr/local/bin/cat"), "cat");
    }

    #[cfg(windows)]
    #[test]
    fn windows_find_user_command_prefers_native_wrapper_before_extensionless_script() {
        let bin_dir = std::env::temp_dir().join("rubash-native-wrapper-before-script");
        let _ = fs::remove_dir_all(&bin_dir);
        fs::create_dir_all(&bin_dir).unwrap();
        let script = bin_dir.join("code");
        let wrapper = bin_dir.join("code.cmd");
        fs::write(&script, "#!/usr/bin/env sh\n").unwrap();
        fs::write(&wrapper, "@echo off\r\n").unwrap();

        let mut env_vars = HashMap::new();
        env_vars.insert("PATH".to_string(), bin_dir.to_string_lossy().to_string());
        env_vars.insert("PATHEXT".to_string(), ".EXE;.PS1".to_string());

        assert_eq!(find_user_command("code", &env_vars), Some(wrapper));

        let _ = fs::remove_dir_all(bin_dir);
    }

    #[cfg(windows)]
    #[test]
    fn windows_find_user_command_prefers_ps1_before_extensionless_script() {
        let bin_dir = std::env::temp_dir().join("rubash-ps1-before-script");
        let _ = fs::remove_dir_all(&bin_dir);
        fs::create_dir_all(&bin_dir).unwrap();
        let script = bin_dir.join("tool");
        let wrapper = bin_dir.join("tool.ps1");
        fs::write(&script, "#!/usr/bin/env sh\n").unwrap();
        fs::write(&wrapper, "Write-Output tool\r\n").unwrap();

        let mut env_vars = HashMap::new();
        env_vars.insert("PATH".to_string(), bin_dir.to_string_lossy().to_string());
        env_vars.insert("PATHEXT".to_string(), ".EXE".to_string());

        assert_eq!(find_user_command("tool", &env_vars), Some(wrapper));

        let _ = fs::remove_dir_all(bin_dir);
    }

    #[cfg(windows)]
    #[test]
    fn windows_extensionless_script_without_sh_falls_back_to_current_shell() {
        let bin_dir = std::env::temp_dir().join("rubash-extensionless-self-shell");
        let _ = fs::remove_dir_all(&bin_dir);
        fs::create_dir_all(&bin_dir).unwrap();
        let script = bin_dir.join("code");
        fs::write(&script, "#!/usr/bin/env sh\n").unwrap();

        let mut env_vars = HashMap::new();
        env_vars.insert("PATH".to_string(), String::new());

        let (command, used_shell) = external_command_for_program(&script, &[".".into()], &env_vars);
        let args = command
            .get_args()
            .map(|arg| arg.to_string_lossy().to_string())
            .collect::<Vec<_>>();

        assert!(used_shell);
        assert_eq!(
            PathBuf::from(command.get_program()),
            std::env::current_exe().unwrap()
        );
        assert_eq!(
            args,
            vec![script.to_string_lossy().to_string(), ".".to_string()]
        );

        let _ = fs::remove_dir_all(bin_dir);
    }

    #[cfg(windows)]
    #[test]
    fn windows_ps1_commands_run_through_powershell_file() {
        let bin_dir = std::env::temp_dir().join("rubash-powershell-bin");
        let _ = fs::remove_dir_all(&bin_dir);
        fs::create_dir_all(&bin_dir).unwrap();
        let pwsh = bin_dir.join("pwsh");
        fs::write(&pwsh, "").unwrap();
        let script = bin_dir.join("probe.ps1");
        fs::write(&script, "").unwrap();

        let mut env_vars = HashMap::new();
        env_vars.insert("PATH".to_string(), bin_dir.to_string_lossy().to_string());

        let (command, used_shell) =
            external_command_for_program(&script, &["one".into(), "two".into()], &env_vars);
        let expected_script = cmd_compatible_windows_path(&script)
            .to_string_lossy()
            .to_string();
        let args = command
            .get_args()
            .map(|arg| arg.to_string_lossy().to_string())
            .collect::<Vec<_>>();

        assert!(!used_shell);
        assert_eq!(PathBuf::from(command.get_program()), pwsh);
        assert_eq!(
            args,
            vec![
                "-NoProfile".to_string(),
                "-ExecutionPolicy".to_string(),
                "Bypass".to_string(),
                "-File".to_string(),
                expected_script,
                "one".to_string(),
                "two".to_string(),
            ]
        );

        let _ = fs::remove_dir_all(bin_dir);
    }

    #[cfg(windows)]
    #[test]
    fn windows_external_arguments_translate_shell_display_paths() {
        let env_vars = HashMap::new();
        let (command, used_shell) = external_command_for_program(
            &PathBuf::from("head.exe"),
            &["/c/Users/example/file.txt".to_string()],
            &env_vars,
        );
        let args = command
            .get_args()
            .map(|arg| arg.to_string_lossy().to_string())
            .collect::<Vec<_>>();

        assert!(!used_shell);
        assert_eq!(args, vec![r"C:\Users\example\file.txt"]);
    }

    #[cfg(windows)]
    #[test]
    fn windows_external_arguments_preserve_bare_drive_shaped_patterns() {
        let env_vars = HashMap::new();
        let (command, used_shell) = external_command_for_program(
            &PathBuf::from("git.exe"),
            &["/h/".to_string(), "--literal".to_string()],
            &env_vars,
        );
        let args = command
            .get_args()
            .map(|arg| arg.to_string_lossy().to_string())
            .collect::<Vec<_>>();

        assert!(!used_shell);
        assert_eq!(args, vec!["/h/".to_string(), "--literal".to_string()]);
    }

    #[cfg(windows)]
    #[test]
    fn windows_external_arguments_preserve_nonexistent_drive_shaped_patterns() {
        let env_vars = HashMap::new();
        let (command, used_shell) = external_command_for_program(
            &PathBuf::from("git.exe"),
            &["/h/not-a-real-pathspec".to_string()],
            &env_vars,
        );
        let args = command
            .get_args()
            .map(|arg| arg.to_string_lossy().to_string())
            .collect::<Vec<_>>();

        assert!(!used_shell);
        assert_eq!(args, vec!["/h/not-a-real-pathspec".to_string()]);
    }

    #[cfg(windows)]
    #[test]
    fn windows_dispatcher_receives_original_command_name_before_arguments() {
        let env_vars = HashMap::new();
        let (command, used_shell) = external_command_for_named_program(
            Path::new(r"C:\tools\winuxcmd.exe"),
            Some("head"),
            &["-3000".to_string(), "input.txt".to_string()],
            &env_vars,
        );
        let args = command
            .get_args()
            .map(|arg| arg.to_string_lossy().to_string())
            .collect::<Vec<_>>();

        assert!(!used_shell);
        assert_eq!(args, vec!["head", "-3000", "input.txt"]);
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
