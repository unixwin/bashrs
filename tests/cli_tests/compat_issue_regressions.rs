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
    assert_eq!(
        String::from_utf8_lossy(&output.stdout),
        "\na\nb\na\n"
    );
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
