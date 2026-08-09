use std::io::Write;
use std::process::{Command, Stdio};
use std::{fs, path::Path};

use regex::Regex;

#[path = "cli_tests/examples.rs"]
mod examples;
#[path = "cli_tests/fd_redirects.rs"]
mod fd_redirects;
#[path = "cli_tests/scripts.rs"]
mod scripts;

fn shell_test_path(path: &Path) -> String {
    path.to_string_lossy().replace('\\', "/")
}

fn assert_stderr_matches(stderr: &str, pattern: &str) {
    let regex = Regex::new(pattern).expect("valid regex");
    assert!(
        regex.is_match(stderr),
        "stderr {stderr:?} did not match {pattern:?}"
    );
}

#[test]
fn c_command_reads_named_coproc_stdout_through_array_fd() {
    let output = Command::new(env!("CARGO_BIN_EXE_rubash"))
        .arg("-c")
        .arg(
            "coproc MY { printf 'coproc-ok\\n'; }; read -r out <&\"${MY[0]}\"; echo \"$out\"",
        )
        .output()
        .expect("run rubash");

    assert!(output.status.success());
    assert_eq!(String::from_utf8_lossy(&output.stdout), "coproc-ok\n");
}

#[test]
fn c_command_keeps_named_coproc_output_fds_distinct() {
    let output = Command::new(env!("CARGO_BIN_EXE_rubash"))
        .arg("-c")
        .arg(
            "coproc FIRST { printf 'first\\n'; }; coproc SECOND { printf 'second\\n'; }; \
             read -r first <&\"${FIRST[0]}\"; read -r second <&\"${SECOND[0]}\"; \
             printf '%s:%s\\n' \"$first\" \"$second\"",
        )
        .output()
        .expect("run rubash");

    assert!(output.status.success());
    assert_eq!(String::from_utf8_lossy(&output.stdout), "first:second\n");
}

#[test]
fn c_command_writes_to_named_coproc_stdin_fd() {
    let output = Command::new(env!("CARGO_BIN_EXE_rubash"))
        .arg("-c")
        .arg(
            "coproc WORKER { read -r value; printf 'got:%s\\n' \"$value\"; }; \
             printf 'hello\\n' >&\"${WORKER[1]}\"; \
             read -r result <&\"${WORKER[0]}\"; echo \"$result\"",
        )
        .output()
        .expect("run rubash");

    assert!(output.status.success());
    assert_eq!(String::from_utf8_lossy(&output.stdout), "got:hello\n");
}

#[test]
fn shopt_print_pipeline_is_captured_before_external_stage() {
    let output = Command::new(env!("CARGO_BIN_EXE_rubash"))
        .arg("-c")
        .arg("shopt -p -o | head -n 3")
        .output()
        .expect("run rubash");

    assert!(output.status.success());
    assert_eq!(String::from_utf8_lossy(&output.stdout).lines().count(), 3);
}

#[test]
fn option_and_stack_builtin_pipelines_are_captured() {
    let output = Command::new(env!("CARGO_BIN_EXE_rubash"))
        .arg("-c")
        .arg("umask -S | head -n 1; kill -l | head -n 1; dirs -p | head -n 1")
        .output()
        .expect("run rubash");

    assert!(output.status.success());
    assert_eq!(String::from_utf8_lossy(&output.stdout).lines().count(), 3);
}

#[test]
fn times_limits_and_enable_pipeline_output_is_captured() {
    let output = Command::new(env!("CARGO_BIN_EXE_rubash"))
        .arg("-c")
        .arg("times | head -n 1; ulimit -a | head -n 1; enable -a | head -n 1")
        .output()
        .expect("run rubash");

    assert!(output.status.success());
    assert_eq!(String::from_utf8_lossy(&output.stdout).lines().count(), 3);
}

#[test]
fn declare_pipeline_output_is_captured() {
    let output = Command::new(env!("CARGO_BIN_EXE_rubash"))
        .arg("-c")
        .arg("declare -p | head -n 1")
        .output()
        .expect("run rubash");

    assert!(output.status.success());
    assert_eq!(String::from_utf8_lossy(&output.stdout).lines().count(), 1);
}

#[test]
fn prefix_assignments_reach_env_builtin_pipeline() {
    let output = Command::new(env!("CARGO_BIN_EXE_rubash"))
        .arg("-c")
        .arg("x=1 y=2 env | grep '^x='; x=1 y=2 env | grep '^y='")
        .output()
        .expect("run rubash");

    assert!(output.status.success());
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(stdout.contains("x=1"));
    assert!(stdout.contains("y=2"));
}

#[test]
fn umask_symbolic_mode_prints_after_setting_mode() {
    let output = Command::new(env!("CARGO_BIN_EXE_rubash"))
        .arg("-c")
        .arg("umask -S 0002")
        .output()
        .expect("run rubash");

    assert!(output.status.success());
    assert_eq!(String::from_utf8_lossy(&output.stdout), "u=rwx,g=rwx,o=rx\n");
}

#[test]
fn malformed_pipeline_and_if_are_syntax_errors() {
    for command in [
        "echo hi |",
        "echo hi &&",
        "echo hi & && echo x",
        "if then; fi; echo after",
        "while; do :; done",
        "case x in x) ;;",
        "( echo hi",
        "{ echo hi;",
    ] {
        let output = Command::new(env!("CARGO_BIN_EXE_rubash"))
            .arg("-c")
            .arg(command)
            .output()
            .expect("run rubash");
        assert!(!output.status.success(), "command unexpectedly succeeded: {command}");
        assert!(String::from_utf8_lossy(&output.stderr).contains("syntax error"));
    }
}

#[test]
fn help_and_trap_pipeline_output_is_captured() {
    let output = Command::new(env!("CARGO_BIN_EXE_rubash"))
        .arg("-c")
        .arg("help | head -n 1; trap -l | head -n 1")
        .output()
        .expect("run rubash");

    assert!(output.status.success());
    assert_eq!(String::from_utf8_lossy(&output.stdout).lines().count(), 2);
}

#[test]
#[cfg(windows)]
fn external_pipeline_does_not_buffer_an_unbounded_producer() {
    let output = Command::new(env!("CARGO_BIN_EXE_rubash"))
        .arg("-c")
        .arg("yes pipeline | head -n 3")
        .output()
        .expect("run rubash");

    assert!(output.status.success());
    assert_eq!(
        String::from_utf8_lossy(&output.stdout),
        "pipeline\npipeline\npipeline\n"
    );
}

#[test]
#[cfg(windows)]
fn windows_userprofile_supplies_home_when_home_is_absent() {
    let output = Command::new(env!("CARGO_BIN_EXE_rubash"))
        .env_remove("HOME")
        .env("USERPROFILE", r"C:\rubash-home-test")
        .arg("-c")
        .arg("printf '%s\\n' \"$HOME\"")
        .output()
        .expect("run rubash");

    assert!(output.status.success());
    assert_eq!(
        String::from_utf8_lossy(&output.stdout),
        "C:\\rubash-home-test\n"
    );
}

fn posix_real_seconds(stderr: &str) -> f64 {
    let real = stderr
        .lines()
        .find_map(|line| line.strip_prefix("real "))
        .expect("real time line");
    real.parse::<f64>().expect("numeric real seconds")
}

#[test]
fn bash_execution_string_reflects_c_command() {
    let command = "printf '%s\\n' \"$BASH_EXECUTION_STRING\"";
    let output = Command::new(env!("CARGO_BIN_EXE_rubash"))
        .arg("-c")
        .arg(command)
        .output()
        .expect("run rubash");

    assert!(output.status.success());
    assert_eq!(
        String::from_utf8_lossy(&output.stdout),
        format!("{command}\n")
    );
}

#[test]
fn c_command_uses_command_name_and_positional_arguments() {
    let output = Command::new(env!("CARGO_BIN_EXE_rubash"))
        .arg("-c")
        .arg("printf '%s:%s:%s\\n' \"$0\" \"$1\" \"$#\"")
        .arg("arg0")
        .arg("alpha")
        .arg("beta")
        .output()
        .expect("run rubash");

    assert!(output.status.success());
    assert_eq!(String::from_utf8_lossy(&output.stdout), "arg0:alpha:2\n");
}

#[test]
fn select_menu_uses_bash_stderr_format() {
    let output = Command::new(env!("CARGO_BIN_EXE_rubash"))
        .arg("-c")
        .arg("select x; do printf '<%s>\\n' \"$x\"; break; done <<< 2")
        .arg("arg0")
        .arg("alpha")
        .arg("beta")
        .output()
        .expect("run rubash");

    assert!(output.status.success());
    assert_eq!(String::from_utf8_lossy(&output.stdout), "<beta>\n");
    assert_eq!(
        String::from_utf8_lossy(&output.stderr),
        "1) alpha\n2) beta\n#? "
    );
}

#[test]
fn time_uses_timeformat_variable() {
    let output = Command::new(env!("CARGO_BIN_EXE_rubash"))
        .arg("-c")
        .arg("TIMEFORMAT='elapsed:%R cpu:%P percent:%%'; time true")
        .output()
        .expect("run rubash");

    assert!(output.status.success());
    assert_eq!(String::from_utf8_lossy(&output.stdout), "");
    assert_stderr_matches(
        &String::from_utf8_lossy(&output.stderr),
        r"^elapsed:\d+\.\d{3} cpu:0\.00 percent:%\n$",
    );
}

#[test]
fn time_p_ignores_timeformat_variable() {
    let output = Command::new(env!("CARGO_BIN_EXE_rubash"))
        .arg("-c")
        .arg("TIMEFORMAT='elapsed:%R'; time -p true")
        .output()
        .expect("run rubash");

    assert!(output.status.success());
    assert_eq!(String::from_utf8_lossy(&output.stdout), "");
    assert_stderr_matches(
        &String::from_utf8_lossy(&output.stderr),
        r"^real \d+\.\d{2}\nuser 0\.00\nsys 0\.00\n$",
    );
}

#[test]
fn timeformat_supports_precision_and_long_modifiers() {
    let output = Command::new(env!("CARGO_BIN_EXE_rubash"))
        .arg("-c")
        .arg("TIMEFORMAT='r:%3R u:%2U s:%0S long:%2lR p:%P'; time true")
        .output()
        .expect("run rubash");

    assert!(output.status.success());
    assert_eq!(String::from_utf8_lossy(&output.stdout), "");
    assert_stderr_matches(
        &String::from_utf8_lossy(&output.stderr),
        r"^r:\d+\.\d{3} u:0\.00 s:0 long:\d+m\d+\.\d{2}s p:0\.00\n$",
    );
}

#[test]
fn time_reports_elapsed_wall_clock_for_slow_command() {
    let slow_command = if cfg!(windows) {
        "powershell.exe -NoProfile -Command Start-Sleep -Milliseconds 650"
    } else {
        "sh -c 'sleep 0.65'"
    };
    let output = Command::new(env!("CARGO_BIN_EXE_rubash"))
        .arg("-c")
        .arg(format!("time -p {slow_command}"))
        .output()
        .expect("run rubash");

    assert!(output.status.success());
    assert_eq!(String::from_utf8_lossy(&output.stdout), "");
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert_stderr_matches(&stderr, r"^real \d+\.\d{2}\nuser 0\.00\nsys 0\.00\n$");
    assert!(
        posix_real_seconds(&stderr) >= 0.50,
        "expected non-zero wall time, got stderr {stderr:?}"
    );
}

#[test]
fn timeformat_reports_invalid_format_character() {
    let output = Command::new(env!("CARGO_BIN_EXE_rubash"))
        .arg("-c")
        .arg("TIMEFORMAT='bad:%Z'; time true; echo status:$?")
        .output()
        .expect("run rubash");

    assert!(output.status.success());
    assert_eq!(String::from_utf8_lossy(&output.stdout), "status:0\n");
    assert_eq!(
        String::from_utf8_lossy(&output.stderr),
        "rubash: TIMEFORMAT: `Z': invalid format character\n"
    );
}

#[test]
fn timeformat_rejects_precision_on_percent_cpu() {
    let output = Command::new(env!("CARGO_BIN_EXE_rubash"))
        .arg("-c")
        .arg("TIMEFORMAT='bad:%3P'; time true; echo status:$?")
        .output()
        .expect("run rubash");

    assert!(output.status.success());
    assert_eq!(String::from_utf8_lossy(&output.stdout), "status:0\n");
    assert_eq!(
        String::from_utf8_lossy(&output.stderr),
        "rubash: TIMEFORMAT: `P': invalid format character\n"
    );
}

#[test]
fn c_command_redirects_stdout_to_stderr_fd() {
    let output = Command::new(env!("CARGO_BIN_EXE_rubash"))
        .arg("-c")
        .arg("echo -n '' 1>&2")
        .output()
        .expect("run rubash");

    assert!(output.status.success());
    assert_eq!(String::from_utf8_lossy(&output.stdout), "");
    assert_eq!(String::from_utf8_lossy(&output.stderr), "");
}

#[test]
fn c_command_redirects_stdout_with_default_fd_duplication() {
    let output = Command::new(env!("CARGO_BIN_EXE_rubash"))
        .arg("-c")
        .arg("echo -n hi >&2")
        .output()
        .expect("run rubash");

    assert!(output.status.success());
    assert_eq!(String::from_utf8_lossy(&output.stdout), "");
    assert_eq!(String::from_utf8_lossy(&output.stderr), "hi");
}

#[test]
fn c_command_exec_numeric_fd_copies_default_stdout() {
    let output = Command::new(env!("CARGO_BIN_EXE_rubash"))
        .arg("-c")
        .arg("exec 3>&1; echo via-fd >&3")
        .output()
        .expect("run rubash");

    assert!(output.status.success());
    assert_eq!(String::from_utf8_lossy(&output.stdout), "via-fd\n");
    assert_eq!(String::from_utf8_lossy(&output.stderr), "");
}

#[test]
fn c_command_exec_numeric_fd_copies_default_stderr() {
    let output = Command::new(env!("CARGO_BIN_EXE_rubash"))
        .arg("-c")
        .arg("exec 3>&2; echo via-fd >&3")
        .output()
        .expect("run rubash");

    assert!(output.status.success());
    assert_eq!(String::from_utf8_lossy(&output.stdout), "");
    assert_eq!(String::from_utf8_lossy(&output.stderr), "via-fd\n");
}

#[test]
fn c_command_printf_uses_persistent_fd_copied_from_stdout() {
    let output = Command::new(env!("CARGO_BIN_EXE_rubash"))
        .arg("-c")
        .arg("exec 3>&1; printf '%s\\n' via-fd >&3")
        .output()
        .expect("run rubash");

    assert!(output.status.success());
    assert_eq!(String::from_utf8_lossy(&output.stdout), "via-fd\n");
    assert_eq!(String::from_utf8_lossy(&output.stderr), "");
}

#[test]
fn c_command_printf_uses_persistent_fd_copied_from_stderr() {
    let output = Command::new(env!("CARGO_BIN_EXE_rubash"))
        .arg("-c")
        .arg("exec 3>&2; printf '%s\\n' via-fd >&3")
        .output()
        .expect("run rubash");

    assert!(output.status.success());
    assert_eq!(String::from_utf8_lossy(&output.stdout), "");
    assert_eq!(String::from_utf8_lossy(&output.stderr), "via-fd\n");
}

#[test]
fn c_command_exec_numeric_fd_copies_default_stdin_for_read_u() {
    let mut child = Command::new(env!("CARGO_BIN_EXE_rubash"))
        .arg("-c")
        .arg("exec 3<&0; read -u 3 value; printf '<%s>:%s\\n' \"$value\" \"$?\"")
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .expect("run rubash");

    child
        .stdin
        .as_mut()
        .unwrap()
        .write_all(b"from-stdin\n")
        .unwrap();
    let output = child.wait_with_output().expect("wait rubash");

    assert!(output.status.success());
    assert_eq!(String::from_utf8_lossy(&output.stdout), "<from-stdin>:0\n");
    assert_eq!(String::from_utf8_lossy(&output.stderr), "");
}

#[test]
fn c_command_external_uses_persistent_fd_copied_from_stdin() {
    let rubash = shell_test_path(Path::new(env!("CARGO_BIN_EXE_rubash")));
    let mut child = Command::new(env!("CARGO_BIN_EXE_rubash"))
        .arg("-c")
        .arg(format!("exec 3<&0; {rubash} -c 'cat' <&3"))
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .expect("run rubash");

    child
        .stdin
        .as_mut()
        .unwrap()
        .write_all(b"external-stdin\n")
        .unwrap();
    let output = child.wait_with_output().expect("wait rubash");

    assert!(output.status.success());
    assert_eq!(String::from_utf8_lossy(&output.stdout), "external-stdin\n");
    assert_eq!(String::from_utf8_lossy(&output.stderr), "");
}

#[test]
fn c_command_mapfile_uses_persistent_fd_copied_from_stdin() {
    let mut child = Command::new(env!("CARGO_BIN_EXE_rubash"))
        .arg("-c")
        .arg(
            "exec 3<&0; mapfile -u 3 -t arr; printf '%s:%s:%s\\n' \"${#arr[@]}\" \"${arr[0]}\" \"${arr[1]}\"",
        )
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .expect("run rubash");

    child
        .stdin
        .as_mut()
        .unwrap()
        .write_all(b"alpha\nbeta\n")
        .unwrap();
    let output = child.wait_with_output().expect("wait rubash");

    assert!(output.status.success());
    assert_eq!(String::from_utf8_lossy(&output.stdout), "2:alpha:beta\n");
    assert_eq!(String::from_utf8_lossy(&output.stderr), "");
}

#[test]
fn script_file_uses_script_name_and_positional_arguments() {
    let script_path = Path::new("target").join("rubash-cli-script-args.sh");
    fs::create_dir_all("target").unwrap();
    fs::write(&script_path, "printf '%s:%s:%s\\n' \"$0\" \"$1\" \"$#\"\n").unwrap();
    let script = script_path.to_string_lossy().to_string();
    let output = Command::new(env!("CARGO_BIN_EXE_rubash"))
        .arg(&script)
        .arg("alpha")
        .arg("beta")
        .output()
        .expect("run rubash");

    assert!(output.status.success());
    assert_eq!(
        String::from_utf8_lossy(&output.stdout),
        format!("{script}:alpha:2\n")
    );
    let _ = fs::remove_file(script_path);
}

#[test]
fn script_file_accepts_shell_style_drive_path() {
    let script_path = Path::new("target").join("rubash-cli-shell-drive-path.sh");
    fs::create_dir_all("target").unwrap();
    fs::write(&script_path, "printf '%s\\n' \"$0\"\n").unwrap();
    let shell_path = shell_test_path(&std::env::current_dir().unwrap().join(&script_path));
    let output = Command::new(env!("CARGO_BIN_EXE_rubash"))
        .arg(&shell_path)
        .output()
        .expect("run rubash");

    assert!(output.status.success());
    assert_eq!(
        String::from_utf8_lossy(&output.stdout),
        format!("{shell_path}\n")
    );
    let _ = fs::remove_file(script_path);
}

#[cfg(windows)]
#[test]
fn extensionless_shell_script_on_path_runs_without_external_sh() {
    let bin_dir = Path::new("target").join("rubash-cli-extensionless-bin");
    let _ = fs::remove_dir_all(&bin_dir);
    fs::create_dir_all(&bin_dir).unwrap();
    let script_path = bin_dir.join("tool");
    fs::write(
        &script_path,
        "#!/usr/bin/env sh\nprintf 'script:%s:%s:%s\\n' \"$0\" \"$1\" \"$#\"\n",
    )
    .unwrap();

    let output = Command::new(env!("CARGO_BIN_EXE_rubash"))
        .arg("-c")
        .arg("tool alpha beta")
        .env("PATH", bin_dir.to_string_lossy().to_string())
        .output()
        .expect("run rubash");

    let _ = fs::remove_dir_all(&bin_dir);
    assert!(
        output.status.success(),
        "stdout={}\nstderr={}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
    assert_eq!(
        String::from_utf8_lossy(&output.stdout),
        format!("script:{}:alpha:2\n", script_path.to_string_lossy())
    );
}

#[test]
fn double_dash_allows_script_file_after_options() {
    let script_path = Path::new("target").join("rubash-cli-double-dash-script.sh");
    fs::create_dir_all("target").unwrap();
    fs::write(&script_path, "printf '%s:%s\\n' \"$0\" \"$1\"\n").unwrap();
    let script = script_path.to_string_lossy().to_string();
    let output = Command::new(env!("CARGO_BIN_EXE_rubash"))
        .arg("--")
        .arg(&script)
        .arg("alpha")
        .output()
        .expect("run rubash");

    assert!(output.status.success());
    assert_eq!(
        String::from_utf8_lossy(&output.stdout),
        format!("{script}:alpha\n")
    );
    let _ = fs::remove_file(script_path);
}

#[test]
fn posix_long_option_enables_posix_mode() {
    let output = Command::new(env!("CARGO_BIN_EXE_rubash"))
        .arg("--posix")
        .arg("-c")
        .arg("type break")
        .output()
        .expect("run rubash");

    assert!(output.status.success());
    assert_eq!(
        String::from_utf8_lossy(&output.stdout),
        "break is a special shell builtin\n"
    );
}

#[test]
fn cli_shell_option_name_applies_before_command_string() {
    let output = Command::new(env!("CARGO_BIN_EXE_rubash"))
        .arg("-o")
        .arg("errexit")
        .arg("-c")
        .arg("[[ -o errexit ]]; echo $?")
        .output()
        .expect("run rubash");

    assert!(output.status.success());
    assert_eq!(String::from_utf8_lossy(&output.stdout), "0\n");
}

#[test]
fn cli_plus_shell_option_name_disables_previous_option() {
    let output = Command::new(env!("CARGO_BIN_EXE_rubash"))
        .arg("-e")
        .arg("+o")
        .arg("errexit")
        .arg("-c")
        .arg("[[ -o errexit ]]; echo $?")
        .output()
        .expect("run rubash");

    assert!(output.status.success());
    assert_eq!(String::from_utf8_lossy(&output.stdout), "1\n");
}

#[test]
fn cli_o_posix_sets_posix_mode_and_shell_option() {
    let output = Command::new(env!("CARGO_BIN_EXE_rubash"))
        .arg("-o")
        .arg("posix")
        .arg("-c")
        .arg("type break; [[ -o posix ]]; echo option:$?")
        .output()
        .expect("run rubash");

    assert!(output.status.success());
    assert_eq!(
        String::from_utf8_lossy(&output.stdout),
        "break is a special shell builtin\noption:0\n"
    );
}

#[test]
fn invalid_cli_shell_option_fails_before_command_string() {
    let output = Command::new(env!("CARGO_BIN_EXE_rubash"))
        .arg("-o")
        .arg("no_such_shell_option")
        .arg("-c")
        .arg("echo should-not-run")
        .output()
        .expect("run rubash");

    assert_eq!(output.status.code(), Some(2));
    assert_eq!(String::from_utf8_lossy(&output.stdout), "");
    assert!(String::from_utf8_lossy(&output.stderr).contains("invalid shell option name"));
}

#[test]
fn profile_startup_options_are_accepted_before_command_string() {
    let output = Command::new(env!("CARGO_BIN_EXE_rubash"))
        .arg("--noprofile")
        .arg("--norc")
        .arg("-c")
        .arg("printf '%s\\n' ok")
        .output()
        .expect("run rubash");

    assert!(output.status.success());
    assert_eq!(String::from_utf8_lossy(&output.stdout), "ok\n");
}

#[test]
fn login_startup_options_are_accepted_before_command_string() {
    let long_output = Command::new(env!("CARGO_BIN_EXE_rubash"))
        .arg("--login")
        .arg("-c")
        .arg("printf '%s\\n' long")
        .output()
        .expect("run rubash");
    let short_output = Command::new(env!("CARGO_BIN_EXE_rubash"))
        .arg("-l")
        .arg("-c")
        .arg("printf '%s\\n' short")
        .output()
        .expect("run rubash");

    assert!(long_output.status.success());
    assert_eq!(String::from_utf8_lossy(&long_output.stdout), "long\n");
    assert!(short_output.status.success());
    assert_eq!(String::from_utf8_lossy(&short_output.stdout), "short\n");
}

#[test]
fn cli_shell_flags_apply_before_command_string() {
    let output = Command::new(env!("CARGO_BIN_EXE_rubash"))
        .arg("-u")
        .arg("-c")
        .arg("printf '%s\\n' \"$-\"")
        .output()
        .expect("run rubash");

    assert!(output.status.success());
    assert!(String::from_utf8_lossy(&output.stdout)
        .trim_end()
        .contains('u'));
}

#[test]
fn command_string_sets_c_shell_flag() {
    let output = Command::new(env!("CARGO_BIN_EXE_rubash"))
        .arg("-c")
        .arg("printf '%s\\n' \"$-\"")
        .output()
        .expect("run rubash");

    assert!(output.status.success());
    assert!(String::from_utf8_lossy(&output.stdout)
        .trim_end()
        .contains('c'));
}

#[test]
fn cli_plus_shell_flags_disable_previous_flags() {
    let output = Command::new(env!("CARGO_BIN_EXE_rubash"))
        .arg("-u")
        .arg("+u")
        .arg("-c")
        .arg("printf '%s\\n' \"$-\"")
        .output()
        .expect("run rubash");

    assert!(output.status.success());
    assert!(!String::from_utf8_lossy(&output.stdout)
        .trim_end()
        .contains('u'));
}

#[test]
fn cli_shopt_options_apply_before_command_string() {
    let output = Command::new(env!("CARGO_BIN_EXE_rubash"))
        .arg("-O")
        .arg("nullglob")
        .arg("-c")
        .arg("shopt -q nullglob; echo $?")
        .output()
        .expect("run rubash");

    assert!(output.status.success());
    assert_eq!(String::from_utf8_lossy(&output.stdout), "0\n");
}

#[test]
fn cli_plus_shopt_options_disable_previous_options() {
    let output = Command::new(env!("CARGO_BIN_EXE_rubash"))
        .arg("-O")
        .arg("nullglob")
        .arg("+O")
        .arg("nullglob")
        .arg("-c")
        .arg("shopt -q nullglob; echo $?")
        .output()
        .expect("run rubash");

    assert!(output.status.success());
    assert_eq!(String::from_utf8_lossy(&output.stdout), "1\n");
}

#[test]
fn invalid_cli_shopt_option_fails_before_command_string() {
    let output = Command::new(env!("CARGO_BIN_EXE_rubash"))
        .arg("-O")
        .arg("no_such_shopt")
        .arg("-c")
        .arg("echo should-not-run")
        .output()
        .expect("run rubash");

    assert_eq!(output.status.code(), Some(2));
    assert_eq!(String::from_utf8_lossy(&output.stdout), "");
    assert!(String::from_utf8_lossy(&output.stderr).contains("invalid shell option name"));
}
