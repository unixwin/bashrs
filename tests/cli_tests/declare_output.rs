use std::process::Command;

#[test]
fn declare_double_dash_output_is_reparseable() {
    let output = Command::new(env!("CARGO_BIN_EXE_rubash"))
        .arg("-c")
        .arg(r###"declare -- s=x; out=$(declare -p s); unset s; eval "$out"; declare -p s; printf '<%s>\n' "$s""###)
        .output()
        .expect("run declare double-dash reparse probe");

    assert!(output.status.success());
    assert_eq!(
        String::from_utf8_lossy(&output.stdout),
        "declare -- s=\"x\"\n<x>\n"
    );
    assert_eq!(String::from_utf8_lossy(&output.stderr), "");
}

#[test]
fn read_array_preserves_assignment_fields_from_declare_output() {
    let output = Command::new(env!("CARGO_BIN_EXE_rubash"))
        .arg("-c")
        .arg(r###"printf '%s\n' 'declare -x NAME="value with spaces"' | while read -a words; do printf '<%s><%s><%s>\n' "${words[0]}" "${words[1]}" "${words[2]%%=*}"; done"###)
        .output()
        .expect("run read array assignment field probe");

    assert!(output.status.success());
    assert_eq!(
        String::from_utf8_lossy(&output.stdout),
        "<declare><-x><NAME>\n"
    );
    assert_eq!(String::from_utf8_lossy(&output.stderr), "");
}

#[test]
fn declare_print_multiline_assoc_values_as_single_read_record() {
    let output = Command::new(env!("CARGO_BIN_EXE_rubash"))
        .arg("-c")
        .arg(r###"declare -A h; h[k]="before
declare -ir p=1
after"; typeset -p h | while read -a words; do var_name=${words[2]%%=*}; [[ $var_name == p ]] && echo BAD:p; [[ $var_name == h ]] && echo OK:h; done"###)
        .output()
        .expect("run multiline declare read record probe");

    assert!(output.status.success());
    assert_eq!(String::from_utf8_lossy(&output.stdout), "OK:h\n");
    assert_eq!(String::from_utf8_lossy(&output.stderr), "");
}
