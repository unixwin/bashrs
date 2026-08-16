use std::process::Command;

#[test]
fn escaped_brace_expansion_preserves_literal_suffix() {
    let output = Command::new(env!("CARGO_BIN_EXE_rubash"))
        .arg("-c")
        .arg(r#"echo {x,y,\{a,b,c}}"#)
        .output()
        .expect("run escaped brace expansion");

    assert!(output.status.success());
    assert_eq!(String::from_utf8_lossy(&output.stdout), "x} y} {a} b} c}\n",);
    assert!(String::from_utf8_lossy(&output.stderr).is_empty());
}

#[test]
fn quoted_parameter_assignment_preserves_escaped_space_for_equals() {
    let script = concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/tests/fixtures/parameter_assignment_escaped_space.sh"
    );
    let output = Command::new(env!("CARGO_BIN_EXE_rubash"))
        .arg(script)
        .output()
        .expect("run parameter assignment escape probe");

    assert!(output.status.success());
    assert_eq!(
        String::from_utf8_lossy(&output.stdout),
        "<a\\ b> <x> <a\\ b> \n"
    );
    assert!(String::from_utf8_lossy(&output.stderr).is_empty());
}

#[test]
fn quoted_parameter_pattern_braces_match_bash() {
    let output = Command::new(env!("CARGO_BIN_EXE_rubash"))
        .arg("-c")
        .arg(r#"x=foo*bar; printf '%s\n' "${x##"}"}"; printf '%s\n' "${x##'}'}""#)
        .output()
        .expect("run quoted parameter pattern probe");

    assert!(output.status.success());
    assert_eq!(
        String::from_utf8_lossy(&output.stdout),
        "foo*bar\nfoo*bar\n"
    );
    assert!(String::from_utf8_lossy(&output.stderr).is_empty());
}

#[test]
fn malformed_script_preserves_valid_prefix_before_status_two() {
    let script = concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/tests/fixtures/malformed_parameter_prefix.sh"
    );
    let output = Command::new(env!("CARGO_BIN_EXE_rubash"))
        .arg(script)
        .output()
        .expect("run multiline malformed expansion fixture");

    assert_eq!(output.status.code(), Some(2));
    assert_eq!(String::from_utf8_lossy(&output.stdout), "a\nb\na b\n");
    assert!(String::from_utf8_lossy(&output.stderr).contains("unexpected end of file"));
}

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
fn parameter_replacement_consumes_quoted_backslashes_like_bash() {
    let output = Command::new(env!("CARGO_BIN_EXE_rubash"))
        .arg("-c")
        .arg(r#"value=x; printf '<%s>|<%s>\n' "${value//x/\n}" "${value//x/\\n}""#)
        .output()
        .expect("run parameter replacement escape probe");

    assert!(output.status.success());
    assert_eq!(String::from_utf8_lossy(&output.stdout), "<n>|<\\n>\n");
    assert!(String::from_utf8_lossy(&output.stderr).is_empty());
}

#[test]
fn parameter_replacement_keeps_escaped_command_substitution_literal() {
    let output = Command::new(env!("CARGO_BIN_EXE_rubash"))
        .arg("-c")
        .arg(r#"v=abc; printf '<%s>\n' "${v//a/\$(printf X)}""#)
        .output()
        .expect("run escaped replacement command substitution probe");

    assert!(output.status.success());
    assert_eq!(String::from_utf8_lossy(&output.stdout), "<$(printf X)bc>\n");
    assert!(String::from_utf8_lossy(&output.stderr).is_empty());
}

#[test]
fn printf_integer_conversion_keeps_valid_prefix_before_invalid_suffix() {
    let output = Command::new(env!("CARGO_BIN_EXE_rubash"))
        .arg("-c")
        .arg("printf '%d:%d:%d\\n' '1.2' '08' '10#12'; status=$?; printf 'status=%s\\n' \"$status\"")
        .output()
        .expect("run printf integer prefix probe");

    assert_eq!(output.status.code(), Some(0));
    assert_eq!(
        String::from_utf8_lossy(&output.stdout),
        "1:0:10\nstatus=1\n"
    );
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert_eq!(stderr.matches("invalid number").count(), 3, "stderr: {stderr}");
}

#[test]
fn command_substitution_pipeline_applies_tr_translation() {
    let output = Command::new(env!("CARGO_BIN_EXE_rubash"))
        .arg("-c")
        .arg("value=\"$(printf 'x\\n' | tr x y)\"; printf '<%s>\\n' \"$value\"")
        .output()
        .expect("run command substitution pipeline probe");

    assert!(output.status.success());
    assert_eq!(String::from_utf8_lossy(&output.stdout), "<y>\n");
    assert!(String::from_utf8_lossy(&output.stderr).is_empty());
}

#[test]
fn command_substitution_pipeline_applies_common_filters() {
    let output = Command::new(env!("CARGO_BIN_EXE_rubash"))
        .arg("-c")
        .arg(
            "printf '<%s>|<%s>|<%s>\\n' \"$(printf 'b\\na\\n' | grep a)\" \"$(printf 'b\\na\\n' | head -n 1)\" \"$(printf 'b\\na\\n' | wc -l)\"",
        )
        .output()
        .expect("run command substitution filter probe");

    assert!(output.status.success());
    assert_eq!(String::from_utf8_lossy(&output.stdout), "<a>|<b>|<2>\n");
    assert!(String::from_utf8_lossy(&output.stderr).is_empty());
}

#[test]
fn command_substitution_pipeline_preserves_last_filter_status() {
    let output = Command::new(env!("CARGO_BIN_EXE_rubash"))
        .arg("-c")
        .arg(
            "value=\"$(printf 'x\\n' | grep y)\"; printf 'value=<%s> status=%s\\n' \"$value\" \"$?\"",
        )
        .output()
        .expect("run command substitution status probe");

    assert!(output.status.success());
    assert_eq!(
        String::from_utf8_lossy(&output.stdout),
        "value=<> status=1\n"
    );
    assert!(String::from_utf8_lossy(&output.stderr).is_empty());
}

#[test]
fn command_substitution_pipeline_applies_tail_and_uniq() {
    let output = Command::new(env!("CARGO_BIN_EXE_rubash"))
        .arg("-c")
        .arg(
            "printf '<%s>|<%s>\\n' \"$(printf 'b\\nb\\na\\n' | uniq)\" \"$(printf 'b\\nb\\na\\n' | tail -n 1)\"",
        )
        .output()
        .expect("run command substitution tail/uniq probe");

    assert!(output.status.success());
    assert_eq!(String::from_utf8_lossy(&output.stdout), "<b\na>|<a>\n");
    assert!(String::from_utf8_lossy(&output.stderr).is_empty());
}

#[test]
fn command_substitution_nested_pipeline_expands_tr_ranges() {
    let output = Command::new(env!("CARGO_BIN_EXE_rubash"))
        .arg("-c")
        .arg(
            "inner(){ printf 'inner'; }; outer(){ printf '%s:%s' \"$1\" \"$(printf '%s' \"$1\" | tr a-z A-Z)\"; }; echo \"$(outer \"$(inner)\")\"",
        )
        .output()
        .expect("run nested tr range probe");

    assert!(output.status.success());
    assert_eq!(String::from_utf8_lossy(&output.stdout), "inner:INNER\n");
    assert!(String::from_utf8_lossy(&output.stderr).is_empty());
}

#[test]
fn arithmetic_empty_quoted_operand_with_operator_fails() {
    let output = Command::new(env!("CARGO_BIN_EXE_rubash"))
        .arg("-c")
        .arg("(( 1 - \"\" )); printf 'status=%s\\n' \"$?\"")
        .output()
        .expect("run empty arithmetic operand probe");

    assert!(output.status.success());
    assert_eq!(String::from_utf8_lossy(&output.stdout), "status=1\n");
    assert!(String::from_utf8_lossy(&output.stderr).contains("operand expected"));
}

#[test]
fn arithmetic_empty_array_subscript_defaults_to_zero() {
    let output = Command::new(env!("CARGO_BIN_EXE_rubash"))
        .arg("-c")
        .arg(r#"declare -a a; let a[" "]=13; declare -p a"#)
        .output()
        .expect("run empty array subscript probe");

    assert!(output.status.success());
    assert_eq!(
        String::from_utf8_lossy(&output.stdout),
        "declare -a a=([0]=\"13\")\n"
    );
    assert!(String::from_utf8_lossy(&output.stderr).is_empty());
}

#[test]
fn arithmetic_empty_quoted_array_subscript_fails_outside_let() {
    let command_output = Command::new(env!("CARGO_BIN_EXE_rubash"))
        .arg("-c")
        .arg("declare -a a; (( a[\"\"]=24 )); printf 'status=%s\\n' \"$?\"")
        .output()
        .expect("run empty quoted arithmetic command subscript probe");

    assert!(
        command_output.status.success(),
        "status={:?}, stdout={:?}, stderr={:?}",
        command_output.status.code(),
        String::from_utf8_lossy(&command_output.stdout),
        String::from_utf8_lossy(&command_output.stderr)
    );
    assert_eq!(String::from_utf8_lossy(&command_output.stdout), "status=1\n");

    let expansion_output = Command::new(env!("CARGO_BIN_EXE_rubash"))
        .arg("-c")
        .arg("declare -a a; : $(( a[\"\"]=25 )); echo after")
        .output()
        .expect("run empty quoted arithmetic expansion subscript probe");

    assert_eq!(expansion_output.status.code(), Some(1));
    assert_eq!(String::from_utf8_lossy(&expansion_output.stdout), "");
    assert!(!String::from_utf8_lossy(&expansion_output.stderr).is_empty());
}

#[test]
fn parameter_substring_empty_length_is_zero() {
    let output = Command::new(env!("CARGO_BIN_EXE_rubash"))
        .arg("-c")
        .arg(
            "v=12345; printf '<%s>|<%s>|<%s>\\n' \
             \"${v:2:}\" \"${v::}\" \"${v:2}\"",
        )
        .output()
        .expect("run empty parameter substring length probe");

    assert!(output.status.success());
    assert_eq!(
        String::from_utf8_lossy(&output.stdout),
        "<>|<>|<345>\n"
    );
    assert!(String::from_utf8_lossy(&output.stderr).is_empty());
}

#[test]
fn umask_symbolic_output_takes_precedence_over_reusable_output() {
    let output = Command::new(env!("CARGO_BIN_EXE_rubash"))
        .arg("-c")
        .arg("umask -Sp 0002")
        .output()
        .expect("run umask option precedence probe");

    assert!(output.status.success());
    assert_eq!(
        String::from_utf8_lossy(&output.stdout),
        "u=rwx,g=rwx,o=rx\n"
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
        .arg("shopt -s expand_aliases\nalias ll=\"echo hi\"\nll\n")
        .output()
        .expect("run rubash");

    assert!(output.status.success());
    assert_eq!(String::from_utf8_lossy(&output.stdout), "hi\n");
    assert!(String::from_utf8_lossy(&output.stderr).is_empty());
}

#[test]
fn aliases_defined_on_the_same_line_are_not_expanded() {
    let output = Command::new(env!("CARGO_BIN_EXE_rubash"))
        .arg("-c")
        .arg(r#"shopt -s expand_aliases; alias ll="echo hi"; ll"#)
        .output()
        .expect("run same-line alias probe");

    assert_eq!(output.status.code(), Some(127));
    assert_eq!(String::from_utf8_lossy(&output.stdout), "");
    assert!(String::from_utf8_lossy(&output.stderr).contains("ll: command not found"));
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
    assert!(
        stderr.contains("value too great for base"),
        "stderr: {stderr}"
    );
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
    assert!(
        String::from_utf8_lossy(&output.stderr).contains("attempted assignment to non-variable")
    );
}

#[test]
fn arithmetic_conditional_requires_false_branch_expression() {
    let output = Command::new(env!("CARGO_BIN_EXE_rubash"))
        .arg("-c")
        .arg("echo $((4 ? 20 : )); echo after")
        .output()
        .expect("run empty arithmetic conditional probe");

    assert_eq!(output.status.code(), Some(1));
    assert_eq!(String::from_utf8_lossy(&output.stdout), "");
    assert!(String::from_utf8_lossy(&output.stderr).contains("syntax error"));
}

#[test]
fn arithmetic_logical_short_circuit_still_parses_bare_assignment() {
    let output = Command::new(env!("CARGO_BIN_EXE_rubash"))
        .arg("-c")
        .arg("B=9; echo $((0 && B=42)); echo after")
        .output()
        .expect("run arithmetic short-circuit assignment probe");

    assert_eq!(output.status.code(), Some(1));
    assert_eq!(String::from_utf8_lossy(&output.stdout), "");
    assert!(!String::from_utf8_lossy(&output.stderr).is_empty());
}

#[test]
fn separated_double_parentheses_parse_as_nested_subshells() {
    let output = Command::new(env!("CARGO_BIN_EXE_rubash"))
        .arg("-c")
        .arg("((echo abc; echo def;); echo ghi); echo after")
        .output()
        .expect("run nested subshell disambiguation probe");

    assert_eq!(output.status.code(), Some(0));
    assert_eq!(
        String::from_utf8_lossy(&output.stdout),
        "abc\ndef\nghi\nafter\n"
    );
}
