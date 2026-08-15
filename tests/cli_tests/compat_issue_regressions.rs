use std::process::Command;

#[test]
fn nested_parameter_pattern_removal_keeps_argument_boundaries() {
    let output = Command::new(env!("CARGO_BIN_EXE_rubash"))
        .arg("-c")
        .arg(
            r##"v=a
echo "${v#?}"
echo "${v%"${v#?}"}"
v=ab
echo "${v#?}"
echo "${v%"${v#?}"}"
"##,
        )
        .output()
        .expect("run rubash");

    assert!(output.status.success());
    assert_eq!(String::from_utf8_lossy(&output.stdout), "\na\nb\na\n");
    assert!(String::from_utf8_lossy(&output.stderr).is_empty());
}

#[test]
fn custom_space_ifs_does_not_create_empty_fields() {
    let output = Command::new(env!("CARGO_BIN_EXE_rubash"))
        .arg("-c")
        .arg(r#"value="a  b"; IFS=" "; printf '<%s>\n' $value"#)
        .output()
        .expect("run rubash");

    assert!(output.status.success());
    assert_eq!(String::from_utf8_lossy(&output.stdout), "<a>\n<b>\n");
    assert!(String::from_utf8_lossy(&output.stderr).is_empty());
}

#[test]
fn arithmetic_nounset_errors_exit_127_like_bash() {
    for command in [
        "set -u; ((missing + 1))",
        r#"set -u; printf '%s\n' "$((missing + 1))""#,
    ] {
        let output = Command::new(env!("CARGO_BIN_EXE_rubash"))
            .arg("-c")
            .arg(command)
            .output()
            .expect("run rubash");

        assert_eq!(output.status.code(), Some(127), "command: {command}");
        assert!(String::from_utf8_lossy(&output.stderr).contains("missing: unbound variable"));
    }
}

#[test]
fn aliases_stay_disabled_in_noninteractive_shells_by_default() {
    let output = Command::new(env!("CARGO_BIN_EXE_rubash"))
        .arg("-c")
        .arg(r#"alias ll="echo hi"; ll"#)
        .output()
        .expect("run rubash");

    assert_eq!(output.status.code(), Some(127));
    assert_eq!(String::from_utf8_lossy(&output.stdout), "");
    assert!(String::from_utf8_lossy(&output.stderr).contains("ll: command not found"));
}

#[test]
fn aliases_expand_when_expand_aliases_is_enabled() {
    let output = Command::new(env!("CARGO_BIN_EXE_rubash"))
        .arg("-c")
        .arg(r#"shopt -s expand_aliases; alias ll="echo hi"; ll"#)
        .output()
        .expect("run rubash");

    assert!(output.status.success());
    assert_eq!(String::from_utf8_lossy(&output.stdout), "hi\n");
    assert!(String::from_utf8_lossy(&output.stderr).is_empty());
}

#[test]
fn arithmetic_conditional_false_branch_assignment_matches_bash() {
    let output = Command::new(env!("CARGO_BIN_EXE_rubash"))
        .arg("-c")
        .arg("x=0; (( 1 ? x=4 : x=9 )); printf 'status:%s x:%s\n' \"$?\" \"$x\"")
        .output()
        .expect("run rubash");

    assert_eq!(String::from_utf8_lossy(&output.stdout), "status:1 x:4\n");
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(
        stderr.contains("attempted assignment to non-variable"),
        "stderr: {stderr}"
    );
    assert!(stderr.contains("error token is \"=9"), "stderr: {stderr}");
}

#[test]
fn arithmetic_invalid_octal_reports_bash_base_diagnostic() {
    let output = Command::new(env!("CARGO_BIN_EXE_rubash"))
        .arg("-c")
        .arg("printf '%s\n' $((08))")
        .output()
        .expect("run invalid octal arithmetic probe");

    assert_eq!(output.status.code(), Some(1));
    assert_eq!(String::from_utf8_lossy(&output.stdout), "");
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(stderr.contains("value too great for base"), "stderr: {stderr}");
    assert!(stderr.contains("error token is \"08\""), "stderr: {stderr}");
}

#[test]
fn inherit_errexit_aborts_command_substitution_body() {
    let output = Command::new(env!("CARGO_BIN_EXE_rubash"))
        .arg("-c")
        .arg("set -e; shopt -s inherit_errexit; x=$(false; echo sub); echo after")
        .output()
        .expect("run inherit_errexit probe");

    assert_eq!(output.status.code(), Some(1));
    assert_eq!(String::from_utf8_lossy(&output.stdout), "");
}

#[test]
fn arithmetic_assignment_error_continues_without_errexit() {
    let output = Command::new(env!("CARGO_BIN_EXE_rubash"))
        .arg("-c")
        .arg("declare -i x=2; y=$((1 ? 20 : x+=2)); echo after:$? y:$y x:$x")
        .output()
        .expect("run nonfatal arithmetic assignment probe");

    assert_eq!(output.status.code(), Some(0));
    assert_eq!(String::from_utf8_lossy(&output.stdout), "after:1 y: x:2\n");
    assert!(String::from_utf8_lossy(&output.stderr)
        .contains("attempted assignment to non-variable"));
}
