//! Thin Windows-compatible bash entry point for Winuxsh installations.
//!
//! The shim intentionally forwards the command line and standard handles
//! without interpreting shell syntax. It lets Unix scripts resolve
//! `/usr/bin/bash` to the installed Winuxsh executable.

use std::env;
use std::io::{self, Write};
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};

fn main() {
    let Some(shell) = locate_winuxsh() else {
        let _ = writeln!(
            io::stderr(),
            "bash shim: cannot locate winuxsh.exe; set WINUXSH_SHELL to its path",
        );
        std::process::exit(127);
    };

    let status = Command::new(&shell)
        .args(env::args_os().skip(1))
        .stdin(Stdio::inherit())
        .stdout(Stdio::inherit())
        .stderr(Stdio::inherit())
        .status();

    match status {
        Ok(status) => std::process::exit(status.code().unwrap_or(1)),
        Err(error) => {
            let _ = writeln!(
                io::stderr(),
                "bash shim: failed to start {}: {error}",
                shell.display()
            );
            std::process::exit(126);
        }
    }
}

fn locate_winuxsh() -> Option<PathBuf> {
    if let Some(path) = env::var_os("WINUXSH_SHELL").map(PathBuf::from) {
        if is_executable_file(&path) && !same_as_current_exe(&path) {
            return Some(path);
        }
    }

    if let Ok(current) = env::current_exe() {
        let mut directory = current.parent().map(Path::to_path_buf);
        while let Some(path) = directory {
            let candidate = path.join("winuxsh.exe");
            if is_executable_file(&candidate) && !same_as_current_exe(&candidate) {
                return Some(candidate);
            }
            directory = path.parent().map(Path::to_path_buf);
        }
    }

    for variable in ["WINUXSH", "SHELL"] {
        if let Some(path) = env::var_os(variable).map(PathBuf::from) {
            if is_executable_file(&path) && !same_as_current_exe(&path) {
                return Some(path);
            }
        }
    }

    find_on_path("winuxsh.exe").or_else(|| find_on_path("winuxsh"))
}

fn find_on_path(name: &str) -> Option<PathBuf> {
    env::var_os("PATH").and_then(|path| {
        env::split_paths(&path)
            .map(|directory| directory.join(name))
            .find(|candidate| is_executable_file(candidate) && !same_as_current_exe(candidate))
    })
}

fn is_executable_file(path: &Path) -> bool {
    path.is_file()
}

fn same_as_current_exe(path: &Path) -> bool {
    let Ok(current) = env::current_exe() else {
        return false;
    };
    let Ok(candidate) = path.canonicalize() else {
        return false;
    };
    current
        .canonicalize()
        .ok()
        .is_some_and(|current| current == candidate)
}
