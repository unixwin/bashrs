use std::{env, process::Command};

#[test]
fn nested_then_sequential_heredocs_keep_fifo_ownership() {
    let output = Command::new(env!("CARGO_BIN_EXE_rubash"))
        .arg("-c")
        .arg("if true; then cat <<A; fi\none\nA\ncat <<B\ntwo\nB")
        .output()
        .expect("run nested sequential heredoc probe");

    assert!(output.status.success());
    assert_eq!(String::from_utf8_lossy(&output.stdout), "one\ntwo\n");
    assert_eq!(String::from_utf8_lossy(&output.stderr), "");
}
