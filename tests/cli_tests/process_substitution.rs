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
