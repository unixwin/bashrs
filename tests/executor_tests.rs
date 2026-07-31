//! Executor Tests - TDD for Bash Command Executor
//!
//! Run with: cargo test --test executor_tests

use rubash::executor::{ExecuteError, Executor};
use rubash::lexer::tokenize;
use rubash::parser::parse;

fn shell_test_path(path: &std::path::Path) -> String {
    let value = path.to_string_lossy().replace('\\', "/");
    if cfg!(windows) && value.len() >= 3 && value.as_bytes()[1] == b':' {
        let drive = value.as_bytes()[0] as char;
        format!("/{}{}", drive.to_ascii_lowercase(), &value[2..])
    } else {
        value
    }
}

fn shell_output_path_to_host(path: &str) -> std::path::PathBuf {
    if cfg!(windows) && path.len() >= 3 && path.as_bytes()[0] == b'/' && path.as_bytes()[2] == b'/'
    {
        let drive = path.as_bytes()[1] as char;
        return std::path::PathBuf::from(
            format!("{}:\\{}", drive.to_ascii_uppercase(), &path[3..]).replace('/', "\\"),
        );
    }
    std::path::PathBuf::from(path)
}

fn target_test_path(name: &str) -> std::path::PathBuf {
    std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("target")
        .join(name)
}

fn write_executable(
    path: impl AsRef<std::path::Path>,
    contents: impl AsRef<[u8]>,
) -> std::io::Result<()> {
    std::fs::write(path.as_ref(), contents)?;
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;

        let mut permissions = std::fs::metadata(path.as_ref())?.permissions();
        permissions.set_mode(0o755);
        std::fs::set_permissions(path.as_ref(), permissions)?;
    }
    Ok(())
}

fn test_command_path(bin_dir: &str, name: &str) -> String {
    if cfg!(windows) {
        format!("{bin_dir}/{name}.cmd")
    } else {
        format!("{bin_dir}/{name}")
    }
}

fn write_test_command(
    path: impl AsRef<std::path::Path>,
    unix_contents: impl AsRef<[u8]>,
    windows_contents: impl AsRef<[u8]>,
) -> std::io::Result<()> {
    if cfg!(windows) {
        write_executable(path, windows_contents)
    } else {
        write_executable(path, unix_contents)
    }
}

fn read_normalized(path: impl AsRef<std::path::Path>) -> String {
    std::fs::read_to_string(path).unwrap().replace("\r\n", "\n")
}

mod simple_execution {
    use super::*;

    #[test]
    fn test_echo_command() {
        let input = "echo hello";
        let tokens = tokenize(input);
        let ast = parse(&tokens);
        let mut executor = Executor::new();
        let result = executor.execute_ast(&ast);
        assert!(result.is_ok());
    }

    #[test]
    fn test_echo_multiple_args() {
        let input = "echo hello world";
        let tokens = tokenize(input);
        let ast = parse(&tokens);
        let mut executor = Executor::new();
        let result = executor.execute_ast(&ast);
        assert!(result.is_ok());
    }

    #[test]
    fn test_exit_command() {
        let input = "exit 0";
        let tokens = tokenize(input);
        let ast = parse(&tokens);
        let mut executor = Executor::new();
        let result = executor.execute_ast(&ast);
        assert!(result.is_err());
    }

    #[test]
    fn test_pwd_command() {
        let input = "pwd";
        let tokens = tokenize(input);
        let ast = parse(&tokens);
        let mut executor = Executor::new();
        let result = executor.execute_ast(&ast);
        assert!(result.is_ok());
    }

    #[test]
    fn test_true_command() {
        let tokens = tokenize("true");
        let ast = parse(&tokens);
        let mut executor = Executor::new();
        executor.execute_ast(&ast).ok();
        assert_eq!(executor.last_exit_code(), 0);
    }

    #[test]
    fn test_false_command() {
        let tokens = tokenize("false");
        let ast = parse(&tokens);
        let mut executor = Executor::new();
        executor.execute_ast(&ast).ok();
        assert_eq!(executor.last_exit_code(), 1);
    }
}

mod exit_codes {
    use super::*;

    #[test]
    fn test_exit_with_code() {
        let input = "exit 42";
        let tokens = tokenize(input);
        let ast = parse(&tokens);
        let mut executor = Executor::new();
        executor.execute_ast(&ast).unwrap_err();
        assert_eq!(executor.last_exit_code(), 42);
    }

    #[test]
    fn test_exit_without_code() {
        let input = "exit";
        let tokens = tokenize(input);
        let ast = parse(&tokens);
        let mut executor = Executor::new();
        executor.execute_ast(&ast).unwrap_err();
        assert_eq!(executor.last_exit_code(), 0);
    }
}

mod environment_tests {
    use super::*;

    #[test]
    fn test_export_command() {
        let input = "export TEST_VAR=hello";
        let tokens = tokenize(input);
        let ast = parse(&tokens);
        let mut executor = Executor::new();
        let result = executor.execute_ast(&ast);
        assert!(result.is_ok());
    }

    #[test]
    fn test_env_var_storage() {
        let mut executor = Executor::new();
        executor.set_env("MY_VAR", "hello");
        assert_eq!(executor.get_env("MY_VAR"), Some("hello"));
    }

    #[test]
    fn test_unset_command() {
        let input = "unset HOME";
        let tokens = tokenize(input);
        let ast = parse(&tokens);
        let mut executor = Executor::new();
        let result = executor.execute_ast(&ast);
        assert!(result.is_ok());
    }
}

#[cfg(windows)]
mod windows_script_commands {
    use super::*;

    #[test]
    fn dot_sh_path_command_runs_in_rubash_without_leaking_state() {
        let dir = target_test_path("windows-direct-sh-script");
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        let script = dir.join("probe.sh");
        let marker = dir.join("marker.txt");
        write_executable(
            &script,
            format!(
                "echo \"$__RUBASH_SCRIPT_NAME|$1|$#|$BASH_SUBSHELL\" > {}\ncd ..\nexport RUBASH_DIRECT_SCRIPT_LEAK=1\n",
                shell_test_path(&marker)
            ),
        )
        .unwrap();

        let old_cwd = std::env::current_dir().unwrap();
        std::env::set_current_dir(&dir).unwrap();

        let tokens = tokenize("./probe.sh one two");
        let ast = parse(&tokens);
        let mut executor = Executor::new();
        executor.set_env("__RUBASH_SCRIPT_NAME", "winuxsh");
        executor.execute_ast(&ast).unwrap();
        let current_cwd = std::env::current_dir().unwrap();
        std::env::set_current_dir(old_cwd).unwrap();

        assert_eq!(
            std::fs::read_to_string(&marker).unwrap().trim(),
            "./probe.sh|one|2|1"
        );
        assert_eq!(executor.get_env("RUBASH_DIRECT_SCRIPT_LEAK"), None);
        assert_eq!(current_cwd, dir);
    }
}

#[path = "executor_command_chaining/mod.rs"]
mod command_chaining;

mod builtin_commands {
    use super::*;

    #[test]
    fn test_env_builtin() {
        let input = "env";
        let tokens = tokenize(input);
        let ast = parse(&tokens);
        let mut executor = Executor::new();
        let result = executor.execute_ast(&ast);
        assert!(result.is_ok());
    }

    #[test]
    fn test_set_builtin() {
        let input = "set";
        let tokens = tokenize(input);
        let ast = parse(&tokens);
        let mut executor = Executor::new();
        let result = executor.execute_ast(&ast);
        assert!(result.is_ok());
    }

    #[test]
    fn test_test_builtin() {
        let input = "test 1 -eq 1";
        let tokens = tokenize(input);
        let ast = parse(&tokens);
        let mut executor = Executor::new();
        let result = executor.execute_ast(&ast);
        assert!(result.is_ok());
    }

    #[test]
    fn test_bracket_builtin() {
        let input = "[ 1 -eq 1 ]";
        let tokens = tokenize(input);
        let ast = parse(&tokens);
        let mut executor = Executor::new();
        let result = executor.execute_ast(&ast);
        assert!(result.is_ok());
    }
}
