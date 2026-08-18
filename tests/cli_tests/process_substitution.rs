use std::process::Command;

#[test]
fn c_command_exec_persistent_output_process_substitution_receives_later_writes() {
    let output = Command::new(env!("CARGO_BIN_EXE_rubash"))
        .arg("-c")
        .arg("exec 9> >(read -r line; printf 'line=%s\\n' \"$line\"); printf 'hello\\n' >&9; exec 9>&-; wait")
        .output()
        .expect("run rubash");

    assert!(output.status.success());
    assert_eq!(String::from_utf8_lossy(&output.stdout), "line=hello\n");
    assert_eq!(String::from_utf8_lossy(&output.stderr), "");
}

#[test]
fn assignment_output_process_substitution_feeds_external_stdin() {
    let output = Command::new(env!("CARGO_BIN_EXE_rubash"))
        .arg("-c")
        .arg(r#"f() { cat "$1" >"$x"; }; x=>(tr '[:lower:]' '[:upper:]') f <(printf 'hi there\n')"#)
        .output()
        .expect("run rubash");

    assert!(output.status.success());
    assert_eq!(String::from_utf8_lossy(&output.stdout), "HI THERE\n");
    assert_eq!(String::from_utf8_lossy(&output.stderr), "");
}

#[test]
fn exec_replacing_stderr_process_substitution_flushes_previous_target() {
    let output = Command::new(env!("CARGO_BIN_EXE_rubash"))
        .arg("-c")
        .arg("rm -f __rubash_psubst_err.tmp; exec 4>&2; exec 2> >(tee __rubash_psubst_err.tmp); echo hello >&2; exec 2>&4; cat __rubash_psubst_err.tmp; rm -f __rubash_psubst_err.tmp")
        .output()
        .expect("run rubash");

    assert!(output.status.success());
    assert_eq!(String::from_utf8_lossy(&output.stdout), "hello\nhello\n");
    assert_eq!(String::from_utf8_lossy(&output.stderr), "");
}
