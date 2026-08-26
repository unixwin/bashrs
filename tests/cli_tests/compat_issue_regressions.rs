#[cfg(not(windows))]
use std::fs;
use std::{env, process::Command};

#[test]
fn posix_parameter_word_can_contain_single_quoted_closing_brace() {
    let output = Command::new(env!("CARGO_BIN_EXE_rubash"))
        .arg("-c")
        .arg(concat!(
            "set -o posix; (echo 1 $",
            "{IFS+'}'z}) 2>&- || echo failed",
        ))
        .output()
        .expect("run POSIX parameter brace probe");

    assert_eq!(output.status.code(), Some(0));
    assert_eq!(String::from_utf8_lossy(&output.stdout), "1 }z\n");
    assert!(String::from_utf8_lossy(&output.stderr).is_empty());
}

#[test]
fn posix_parameter_word_preserves_single_quoted_closing_brace_in_double_quotes() {
    let output = Command::new(env!("CARGO_BIN_EXE_rubash"))
        .arg("-c")
        .arg("set -o posix; (echo 2 \"${IFS+'}'z}\")")
        .output()
        .expect("run quoted POSIX parameter brace probe");

    assert_eq!(output.status.code(), Some(0));
    assert_eq!(String::from_utf8_lossy(&output.stdout), "2 '}'z\n");
    assert!(String::from_utf8_lossy(&output.stderr).is_empty());
}
#[test]
fn quoted_native_wildcards_and_special_arguments_survive_argv_boundary() {
    let output = Command::new(env!("CARGO_BIN_EXE_rubash"))
        .arg("-c")
        .arg(r#"printf '<%s>\n' "a*b" "q?x" "/CN=test" --send-only"#)
        .output()
        .expect("run native argv literal probe");

    assert!(output.status.success());
    assert_eq!(
        String::from_utf8_lossy(&output.stdout),
        "<a*b>\n<q?x>\n</CN=test>\n<--send-only>\n"
    );
    assert!(String::from_utf8_lossy(&output.stderr).is_empty());
}

#[cfg(windows)]
#[test]
fn rubash_to_powershell_preserves_native_argument_literals() {
    let output = Command::new(env!("CARGO_BIN_EXE_rubash"))
        .arg("-c")
        .arg(
            r#"pwsh -NoProfile -Command 'Write-Output (Get-Variable args -ValueOnly)' -- "a*b" "q?x" "/CN=test" --send-only"#,
        )
        .output()
        .expect("run Rubash to PowerShell argv probe");

    assert!(
        output.status.success(),
        "stderr: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    assert_eq!(
        String::from_utf8_lossy(&output.stdout),
        "a*b\r\nq?x\r\n/CN=test\r\n--send-only\r\n"
    );
    assert!(String::from_utf8_lossy(&output.stderr).is_empty());
}

#[test]
fn quoted_positional_at_keeps_suffix_expansion_unquoted() {
    let output = Command::new(env!("CARGO_BIN_EXE_rubash"))
        .arg("-c")
        .arg(r#"set -- a ""; space=" "; printf '<%s>\n' "$@"$space"#)
        .output()
        .expect("run quoted positional suffix probe");

    assert!(output.status.success());
    assert_eq!(String::from_utf8_lossy(&output.stdout), "<a>\n<>\n");
    assert!(String::from_utf8_lossy(&output.stderr).is_empty());
}

#[test]
fn unquoted_parameter_expansion_splits_on_custom_ifs_empty_fields() {
    let output = Command::new(env!("CARGO_BIN_EXE_rubash"))
        .arg("-c")
        .arg(r#"IFS=":"; x=":a:"; set x $x; shift; printf "[%s](%s)(%s)\n" "$#" "$1" "$2"; IFS=" "; x="  "; set x $x; shift; printf "[%s]\n" "$#""#)
        .output()
        .expect("run custom IFS field-splitting probe");

    assert_eq!(output.status.code(), Some(0));
    assert_eq!(String::from_utf8_lossy(&output.stdout), "[2]()(a)\n[0]\n");
    assert_eq!(String::from_utf8_lossy(&output.stderr), "");
}

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
fn heredoc_parameter_error_writes_to_stderr_and_continues() {
    let output = Command::new(env!("CARGO_BIN_EXE_rubash"))
        .arg("-c")
        .arg("M=AAA; cat <<EOF; echo Y\n${D?$M}\nEOF")
        .output()
        .expect("run heredoc parameter error probe");

    assert_eq!(output.status.code(), Some(0));
    assert_eq!(String::from_utf8_lossy(&output.stdout), "Y\n");
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(stderr.contains("D: AAA"), "stderr was {stderr:?}");
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
        .arg(
            "printf '%d:%d:%d\\n' '1.2' '08' '10#12'; status=$?; printf 'status=%s\\n' \"$status\"",
        )
        .output()
        .expect("run printf integer prefix probe");

    assert_eq!(output.status.code(), Some(0));
    assert_eq!(
        String::from_utf8_lossy(&output.stdout),
        "1:0:10\nstatus=1\n"
    );
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert_eq!(stderr.lines().count(), 3, "stderr: {stderr}");
    assert!(stderr.contains("1.2: invalid number"));
    assert!(stderr.contains("08: invalid octal number"));
    assert!(stderr.contains("10#12: invalid number"));
}

#[test]
fn printf_b_emits_octals_as_raw_bytes() {
    let output = Command::new(env!("CARGO_BIN_EXE_rubash"))
        .arg("-c")
        .arg("printf '%b|%b|%b' '\\400' '\\777' '\\377'")
        .output()
        .expect("run printf raw octal probe");

    assert!(output.status.success());
    assert_eq!(output.stdout, b"\0|\xff|\xff");
    assert!(output.stderr.is_empty());
}

#[test]
fn printf_q_uses_ansi_c_quotes_for_control_bytes() {
    let output = Command::new(env!("CARGO_BIN_EXE_rubash"))
        .arg("-c")
        .arg("printf '<%q>\\n' $'a\\x01b'")
        .output()
        .expect("run printf control quoting probe");

    assert!(output.status.success());
    assert_eq!(String::from_utf8_lossy(&output.stdout), "<$'a\\001b'>\n");
    assert!(String::from_utf8_lossy(&output.stderr).is_empty());
}

#[test]
fn command_substitution_preserves_printf_raw_bytes() {
    let output = Command::new(env!("CARGO_BIN_EXE_rubash"))
        .arg("-c")
        .arg("value=\"$(printf '\\377')\"; printf '%s' \"$value\"")
        .output()
        .expect("run command substitution raw byte probe");

    assert!(output.status.success());
    assert_eq!(output.stdout, b"\xff");
    assert!(output.stderr.is_empty());
}

#[test]
fn timed_command_substitution_preserves_printf_raw_bytes() {
    let output = Command::new(env!("CARGO_BIN_EXE_rubash"))
        .arg("-c")
        .arg("value=\"$(time printf '\\377')\"; printf '%s' \"$value\"")
        .output()
        .expect("run timed substitution raw byte probe");

    assert!(output.status.success());
    assert_eq!(output.stdout, b"\xff");
}

#[test]
fn command_substitution_pipeline_preserves_printf_raw_bytes() {
    let output = Command::new(env!("CARGO_BIN_EXE_rubash"))
        .arg("-c")
        .arg("value=\"$(printf '\\377' | tr x y)\"; printf '%s' \"$value\"")
        .output()
        .expect("run pipeline raw byte probe");

    assert!(output.status.success());
    assert_eq!(output.stdout, b"\xff");
    assert!(output.stderr.is_empty());
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
    assert_eq!(
        String::from_utf8_lossy(&command_output.stdout),
        "status=1\n"
    );

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
    assert_eq!(String::from_utf8_lossy(&output.stdout), "<>|<>|<345>\n");
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
fn quoted_arithmetic_expansion_survives_test_builtin_word_expansion() {
    let output = Command::new(env!("CARGO_BIN_EXE_rubash"))
        .arg("-c")
        .arg("set 4 '' opt no-highlight 0 '' x; if [ \"$(($2 + 2))\" -gt \"$#\" ]; then echo yes; else echo no; fi")
        .output()
        .expect("run quoted arithmetic test operand");
    assert!(output.status.success());
    assert_eq!(String::from_utf8_lossy(&output.stdout), "no\n");
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
fn arithmetic_word_errors_continue_without_errexit() {
    for expression in ["1=2", "1++", "1/0", "08"] {
        let output = Command::new(env!("CARGO_BIN_EXE_rubash"))
            .arg("-c")
            .arg(format!("echo $(({expression})); echo after"))
            .output()
            .expect("run ordinary-word arithmetic probe");

        assert_eq!(output.status.code(), Some(0), "expression: {expression}");
        assert_eq!(
            String::from_utf8_lossy(&output.stdout),
            "after\n",
            "expression: {expression}"
        );
        assert!(
            !output.stderr.is_empty(),
            "expected arithmetic diagnostic for {expression}"
        );
    }
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
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(stderr.contains("attempted assignment to non-variable"));
    assert!(
        stderr.contains("error token is \"+=2\""),
        "stderr: {stderr}"
    );
}

#[test]
fn associative_arithmetic_subscript_preserves_escaped_command_substitution() {
    let output = Command::new(env!("CARGO_BIN_EXE_rubash"))
        .arg("-c")
        .arg("declare -A a; key=\"x],b[\\$(echo uname >&2)\"; (( a[$key]++ )); declare -p a")
        .output()
        .expect("run associative arithmetic subscript probe");

    assert_eq!(output.status.code(), Some(0));
    assert_eq!(
        String::from_utf8_lossy(&output.stdout),
        "declare -A a=([\"x],b[$(echo uname >&2)\"]=\"1\" )\n"
    );
    assert!(String::from_utf8_lossy(&output.stderr).is_empty());
}

#[test]
fn arithmetic_conditional_requires_false_branch_expression() {
    let output = Command::new(env!("CARGO_BIN_EXE_rubash"))
        .arg("-c")
        .arg("echo $((4 ? 20 : )); echo after")
        .output()
        .expect("run empty arithmetic conditional probe");

    assert_eq!(output.status.code(), Some(0));
    assert_eq!(String::from_utf8_lossy(&output.stdout), "after\n");
    assert!(String::from_utf8_lossy(&output.stderr).contains("syntax error"));
}

#[test]
fn nameref_to_array_element_expands_value_and_indirect_name() {
    let output = Command::new(env!("CARGO_BIN_EXE_rubash"))
        .arg("-c")
        .arg("arr=(zero 'one two'); declare -n ref='arr[1]'; printf '<%s>|<%s>\n' \"${ref}\" \"${!ref}\"")
        .output()
        .expect("run nameref array element probe");

    assert!(output.status.success());
    assert_eq!(
        String::from_utf8_lossy(&output.stdout),
        "<one two>|<arr[1]>\n"
    );
    assert!(String::from_utf8_lossy(&output.stderr).is_empty());
}

#[test]
fn positional_slice_offset_zero_includes_script_name() {
    let output = Command::new(env!("CARGO_BIN_EXE_rubash"))
        .arg("-c")
        .arg("set -- alpha beta; got=\"${@:0:1}\"; [ \"$got\" = \"$0\" ]")
        .output()
        .expect("run positional offset-zero probe");

    assert!(
        output.status.success(),
        "stderr: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    assert!(String::from_utf8_lossy(&output.stderr).is_empty());
}

#[test]
fn explicit_read_reply_trims_like_normal_scalar_name() {
    let output = Command::new(env!("CARGO_BIN_EXE_rubash"))
        .arg("-c")
        .arg(r#"printf '%s\n' '  \abc  d\ef  ' | (read REPLY; printf '<%s>\n' "$REPLY")"#)
        .output()
        .expect("run explicit read REPLY probe");

    assert_eq!(output.status.code(), Some(0));
    assert_eq!(String::from_utf8_lossy(&output.stdout), "<abc  def>\n");
    assert_eq!(String::from_utf8_lossy(&output.stderr), "");
}

#[test]
fn read_without_r_joins_backslash_newline() {
    let output = Command::new(env!("CARGO_BIN_EXE_rubash"))
        .arg("-c")
        .arg(r#"printf '%s\n%s\n' 'test\' 'best' | (read reply; printf '<%s>\n' "$reply")"#)
        .output()
        .expect("run read backslash-newline probe");

    assert_eq!(output.status.code(), Some(0));
    assert_eq!(String::from_utf8_lossy(&output.stdout), "<testbest>\n");
    assert_eq!(String::from_utf8_lossy(&output.stderr), "");
}

#[test]
fn read_mixed_ifs_splits_like_bash() {
    let output = Command::new(env!("CARGO_BIN_EXE_rubash"))
        .arg("-c")
        .arg(
            r#"printf 'a ,, c
a ,, c d
	,	a	,,	b	c
' | { IFS=" ," read a b c; printf '<%s><%s><%s>
' "$a" "$b" "$c"; IFS=" ," read a b c d; printf '<%s><%s><%s><%s>
' "$a" "$b" "$c" "$d"; IFS=$(printf ' 	,') read a b c d e; printf '<%s><%s><%s><%s><%s>
' "$a" "$b" "$c" "$d" "$e"; }"#,
        )
        .output()
        .expect("run read mixed IFS probe");

    assert_eq!(output.status.code(), Some(0));
    assert_eq!(
        String::from_utf8_lossy(&output.stdout),
        "<a><><c>
<a><><c><d>
<><a><><b><c>
"
    );
    assert_eq!(String::from_utf8_lossy(&output.stderr), "");
}

#[test]
fn empty_command_substitution_command_word_preserves_previous_status() {
    let output = Command::new(env!("CARGO_BIN_EXE_rubash"))
        .arg("-c")
        .arg(
            r#"true; $(); printf 'a:%s
' "$?"; false; $(); printf 'b:%s
' "$?""#,
        )
        .output()
        .expect("run empty command substitution command-word probe");

    assert_eq!(output.status.code(), Some(0));
    assert_eq!(String::from_utf8_lossy(&output.stdout), "a:0\nb:0\n");
    assert_eq!(String::from_utf8_lossy(&output.stderr), "");
}

#[test]
fn redirection_target_glob_uses_single_match() {
    let output = Command::new(env!("CARGO_BIN_EXE_rubash"))
        .arg("-c")
        .arg(
            r#"rm -f z.tmp; >z.tmp; echo TEST >?.tmp; printf '<%s>\n' "$(cat z.tmp)"; rm -f z.tmp"#,
        )
        .output()
        .expect("run redirection glob probe");

    assert_eq!(output.status.code(), Some(0));
    assert_eq!(String::from_utf8_lossy(&output.stdout), "<TEST>\n");
    assert_eq!(String::from_utf8_lossy(&output.stderr), "");
}

#[cfg(not(windows))]
#[test]
fn posix_redirection_target_does_not_glob() {
    let output = Command::new(env!("CARGO_BIN_EXE_rubash"))
        .arg("--posix")
        .arg("-c")
        .arg(
            r#"rm -f z.tmp '?.tmp'; >z.tmp; echo TEST >?.tmp; printf 'z.tmp:<%s>\n' "$(cat z.tmp)"; printf '?.tmp:<%s>\n' "$(cat '?.tmp')"; rm -f z.tmp '?.tmp'"#,
        )
        .output()
        .expect("run posix redirection glob probe");

    assert_eq!(output.status.code(), Some(0));
    assert_eq!(
        String::from_utf8_lossy(&output.stdout),
        "z.tmp:<>\n?.tmp:<TEST>\n"
    );
    assert_eq!(String::from_utf8_lossy(&output.stderr), "");
}

#[cfg(not(windows))]
#[test]
fn ash_named_invocation_uses_posix_redirection_glob_rules() {
    let dir = env::temp_dir().join(format!("rubash-ash-glob-{}", std::process::id()));
    let _ = fs::remove_dir_all(&dir);
    fs::create_dir_all(&dir).expect("create ash invocation temp dir");
    let ash_path = dir.join(if cfg!(windows) { "ash.exe" } else { "ash" });
    fs::copy(env!("CARGO_BIN_EXE_rubash"), &ash_path).expect("copy rubash as ash");

    let output = Command::new(&ash_path)
        .arg("-c")
        .arg(
            r#"rm -f z.tmp '?.tmp'; >z.tmp; echo TEST >?.tmp; printf 'z.tmp:<%s>\n' "$(cat z.tmp)"; printf '?.tmp:<%s>\n' "$(cat '?.tmp')"; rm -f z.tmp '?.tmp'"#,
        )
        .current_dir(&dir)
        .output()
        .expect("run ash-named redirection glob probe");

    let _ = fs::remove_dir_all(&dir);

    assert_eq!(output.status.code(), Some(0));
    assert_eq!(
        String::from_utf8_lossy(&output.stdout),
        "z.tmp:<>\n?.tmp:<TEST>\n"
    );
    assert_eq!(String::from_utf8_lossy(&output.stderr), "");
}

#[test]
fn glob_backslash_literal_in_unquoted_variable_matches_filename() {
    let output = Command::new(env!("CARGO_BIN_EXE_rubash"))
        .arg("-c")
        .arg(r#"rm -rf testdir.TMP; mkdir testdir.TMP; >testdir.TMP/name; b="test*.TMP/\name"; printf '<%s>\n' $b; rm -f testdir.TMP/name; rmdir testdir.TMP"#)
        .output()
        .expect("run glob backslash variable probe");

    assert_eq!(output.status.code(), Some(0));
    assert_eq!(
        String::from_utf8_lossy(&output.stdout),
        "<testdir.TMP/name>\n"
    );
    assert_eq!(String::from_utf8_lossy(&output.stderr), "");
}

#[test]
fn escaped_glob_from_parameter_expansion_stays_literal() {
    let output = Command::new(env!("CARGO_BIN_EXE_rubash"))
        .arg("-c")
        .arg(r#"rm -f Zf; >Zf; v='\*'; printf '<%s>\n' Z$v; rm -f Zf"#)
        .output()
        .expect("run escaped glob parameter probe");

    assert_eq!(output.status.code(), Some(0));
    assert_eq!(String::from_utf8_lossy(&output.stdout), "<Z\\*>\n");
    assert_eq!(String::from_utf8_lossy(&output.stderr), "");
}

#[test]
fn escaped_glob_in_parameter_alternate_stays_literal() {
    let output = Command::new(env!("CARGO_BIN_EXE_rubash"))
        .arg("-c")
        .arg(r#"rm -f glob_altvalue1.tests; >glob_altvalue1.tests; x=x; printf '<%s>\n' ${x:+glob_altvalue1.t\*}; rm -f glob_altvalue1.tests"#)
        .output()
        .expect("run escaped alternate glob probe");

    assert_eq!(output.status.code(), Some(0));
    assert_eq!(
        String::from_utf8_lossy(&output.stdout),
        "<glob_altvalue1.t*>\n"
    );
    assert_eq!(String::from_utf8_lossy(&output.stderr), "");
}

#[test]
fn parameter_alternate_preserves_nested_literal_double_quotes() {
    let output = Command::new(env!("CARGO_BIN_EXE_rubash"))
        .arg("-c")
        .arg(r#"x=a; printf '<%s>\n' ${x:+"b c" d}; printf '<%s>\n' "${x:+"b c" d}""#)
        .output()
        .expect("run nested quote parameter alternate probe");

    assert_eq!(output.status.code(), Some(0));
    assert_eq!(
        String::from_utf8_lossy(&output.stdout),
        "<b c>\n<d>\n<b c d>\n"
    );
    assert_eq!(String::from_utf8_lossy(&output.stderr), "");
}

#[test]
fn quoted_process_substitution_stays_literal() {
    let output = Command::new(env!("CARGO_BIN_EXE_rubash"))
        .arg("-c")
        .arg("echo \"<(echo \\\"hello 0\\\")\"")
        .output()
        .expect("run quoted process substitution probe");

    assert_eq!(output.status.code(), Some(0));
    assert_eq!(
        String::from_utf8_lossy(&output.stdout),
        "<(echo \"hello 0\")
"
    );
    assert_eq!(String::from_utf8_lossy(&output.stderr), "");
}

#[test]
fn process_substitution_external_redirect_preserves_printf_raw_bytes() {
    let output = Command::new(env!("CARGO_BIN_EXE_rubash"))
        .arg("-c")
        .arg("cat <(printf '\\377')")
        .output()
        .expect("run process substitution external raw byte probe");

    assert_eq!(output.status.code(), Some(0));
    assert_eq!(output.stdout, [0xff]);
    assert_eq!(String::from_utf8_lossy(&output.stderr), "");
}

#[test]
fn command_substitution_preserves_quoted_empty_word() {
    let output = Command::new(env!("CARGO_BIN_EXE_rubash"))
        .arg("-c")
        .arg("set -- \"$(printf '')\"; printf 'q:%d:<%s>\\n' \"$#\" \"$1\"; set -- $(printf ''); printf 'u:%d\\n' \"$#\"")
        .output()
        .expect("run quoted empty substitution probe");

    assert_eq!(output.status.code(), Some(0));
    assert_eq!(String::from_utf8_lossy(&output.stdout), "q:1:<>\nu:0\n");
    assert_eq!(String::from_utf8_lossy(&output.stderr), "");
}

#[test]
fn command_substitution_does_not_split_literal_ifs_bytes() {
    let output = Command::new(env!("CARGO_BIN_EXE_rubash"))
        .arg("-c")
        .arg("IFS=:; set -- a$(printf '')b::c; printf 'ifs:%d:%s:%s:%s:%s\\n' \"$#\" \"$1\" \"$2\" \"$3\" \"$4\"")
        .output()
        .expect("run literal IFS fragment probe");

    assert_eq!(output.status.code(), Some(0));
    assert_eq!(String::from_utf8_lossy(&output.stdout), "ifs:1:ab::c:::\n");
    assert_eq!(String::from_utf8_lossy(&output.stderr), "");
}

#[test]
fn command_substitution_multiple_fragments_split_independently() {
    let output = Command::new(env!("CARGO_BIN_EXE_rubash"))
        .arg("-c")
        .arg(r###"printf '<%s>\n' pre$(printf '%s' a)mid$(printf '%s' 'b c')post; printf '<%s>\n' pre$(printf '%s' a)mid"$(printf '%s' 'b c')"post"###)
        .output()
        .expect("run multiple substitution fragment probe");

    assert_eq!(output.status.code(), Some(0));
    assert_eq!(
        String::from_utf8_lossy(&output.stdout),
        "<preamidb>\n<cpost>\n<preamidb cpost>\n"
    );
    assert_eq!(String::from_utf8_lossy(&output.stderr), "");
}

#[test]
fn command_substitution_mixed_fragments_follow_quote_context() {
    let output = Command::new(env!("CARGO_BIN_EXE_rubash"))
        .arg("-c")
        .arg(r###"printf '<%s>\n' pre$(printf '%s' 'a b')post; printf '<%s>\n' pre"$(printf '%s' 'a b')"post"###)
        .output()
        .expect("run mixed substitution fragment probe");

    assert_eq!(output.status.code(), Some(0));
    assert_eq!(
        String::from_utf8_lossy(&output.stdout),
        "<prea>\n<bpost>\n<prea bpost>\n"
    );
    assert_eq!(String::from_utf8_lossy(&output.stderr), "");
}

#[test]
fn command_substitution_echo_handles_escaped_parens_and_nested_backticks() {
    let output = Command::new(env!("CARGO_BIN_EXE_rubash"))
        .arg("-c")
        .arg(r#"echo $(echo \(\(TEST\) BEST); echo $(echo \)); echo $(echo a"`echo ")"`"c ); echo OK: $?"#)
        .output()
        .expect("run escaped paren command substitution probe");

    assert_eq!(output.status.code(), Some(0));
    assert_eq!(
        String::from_utf8_lossy(&output.stdout),
        "((TEST) BEST
)
a)c
OK: 0
"
    );
    assert_eq!(String::from_utf8_lossy(&output.stderr), "");
}

#[test]
fn quoted_backtick_command_substitution_preserves_newlines_through_pipeline() {
    let output = Command::new(env!("CARGO_BIN_EXE_rubash"))
        .arg("-c")
        .arg(r#"rm -f __rubash_cmdsubst_pipe_lines.tmp; printf '%s\n' one two three > __rubash_cmdsubst_pipe_lines.tmp; printf '<%s>\n' "`cat __rubash_cmdsubst_pipe_lines.tmp`" | cat; rm -f __rubash_cmdsubst_pipe_lines.tmp"#)
        .output()
        .expect("run quoted backtick pipeline newline probe");

    assert_eq!(output.status.code(), Some(0));
    assert_eq!(
        String::from_utf8_lossy(&output.stdout),
        "<one\ntwo\nthree>\n"
    );
    assert_eq!(String::from_utf8_lossy(&output.stderr), "");
}

#[test]
fn quoted_backtick_command_substitution_preserves_internal_newlines() {
    let output = Command::new(env!("CARGO_BIN_EXE_rubash"))
        .arg("-c")
        .arg(r#"rm -f __rubash_cmdsubst_lines.tmp; printf '%s\n' one two three > __rubash_cmdsubst_lines.tmp; printf '<%s>\n' "`cat __rubash_cmdsubst_lines.tmp`"; rm -f __rubash_cmdsubst_lines.tmp"#)
        .output()
        .expect("run quoted backtick newline probe");

    assert_eq!(output.status.code(), Some(0));
    assert_eq!(
        String::from_utf8_lossy(&output.stdout),
        "<one\ntwo\nthree>\n"
    );
    assert_eq!(String::from_utf8_lossy(&output.stderr), "");
}

#[test]
fn assignment_with_null_command_word_uses_assignment_substitution_status() {
    let output = Command::new(env!("CARGO_BIN_EXE_rubash"))
        .arg("-c")
        .arg(r#"v=v; v=`exit 2` `false`; printf 'Two:%s v:[%s]\n' "$?" "$v""#)
        .output()
        .expect("run assignment plus null command word probe");

    assert_eq!(output.status.code(), Some(0));
    assert_eq!(String::from_utf8_lossy(&output.stdout), "Two:2 v:[]\n");
    assert_eq!(String::from_utf8_lossy(&output.stderr), "");
}

#[test]
fn assignment_only_redirect_failure_sets_status_and_continues() {
    let output = Command::new(env!("CARGO_BIN_EXE_rubash"))
        .arg("-c")
        .arg(r#"rm -rf __rubash_missing_redirect_dir__; a=$(exit 2) >__rubash_missing_redirect_dir__/out; printf 'status:%s\n' "$?"; printf 'after\n'"#)
        .output()
        .expect("run assignment-only redirect failure probe");

    assert_eq!(output.status.code(), Some(0));
    assert_eq!(String::from_utf8_lossy(&output.stdout), "status:1\nafter\n");
    assert!(!String::from_utf8_lossy(&output.stderr).is_empty());
}

#[test]
fn failed_empty_output_command_substitution_sets_command_status() {
    let output = Command::new(env!("CARGO_BIN_EXE_rubash"))
        .arg("-c")
        .arg(
            r#"true; $(""); printf 'status:%s
' "$?""#,
        )
        .output()
        .expect("run failed empty-output command substitution probe");

    assert_eq!(output.status.code(), Some(0));
    assert_eq!(
        String::from_utf8_lossy(&output.stdout),
        "status:127
"
    );
    assert!(String::from_utf8_lossy(&output.stderr).contains("command not found"));
}

#[test]
fn read_timeout_keeps_partial_pipeline_input() {
    let output = Command::new(env!("CARGO_BIN_EXE_rubash"))
        .arg("-c")
        .arg(r#"{ echo -n te; sleep 2; echo st; } | (read -t 1 reply; echo ">$reply<")"#)
        .output()
        .expect("run read timeout partial pipeline probe");

    assert_eq!(output.status.code(), Some(0));
    assert_eq!(String::from_utf8_lossy(&output.stdout), ">te<\n");
    assert_eq!(String::from_utf8_lossy(&output.stderr), "");
}

#[test]
fn read_timeout_followup_printf_sees_partial_status() {
    let output = Command::new(env!("CARGO_BIN_EXE_rubash"))
        .arg("-c")
        .arg(r#"{ echo -n te; sleep 2; echo st; } | (read -t 1 reply; printf ">%s<[%s]\n" "$reply" "$?")"#)
        .output()
        .expect("run read timeout printf follow-up probe");

    assert_eq!(output.status.code(), Some(0));
    assert_eq!(String::from_utf8_lossy(&output.stdout), ">te<[142]\n");
    assert_eq!(String::from_utf8_lossy(&output.stderr), "");
}

#[test]
fn read_timeout_zero_reports_pipe_readiness() {
    let output = Command::new(env!("CARGO_BIN_EXE_rubash"))
        .arg("-c")
        .arg(r#"echo Ok | { sleep 0.1; read -t 0 reply; echo ">$reply<[$?]"; }; sleep 0.2 | { read -t 0 reply; echo ">$reply<[$?]"; }"#)
        .output()
        .expect("run read timeout zero readiness probe");

    assert_eq!(output.status.code(), Some(0));
    assert_eq!(String::from_utf8_lossy(&output.stdout), "><[0]\n><[1]\n");
    assert_eq!(String::from_utf8_lossy(&output.stderr), "");
}

#[test]
fn read_prompt_is_suppressed_for_pipeline_input() {
    let output = Command::new(env!("CARGO_BIN_EXE_rubash"))
        .arg("-c")
        .arg(r#"echo data | { read -p IGNORED_PROMPT reply; printf '<%s>\n' "$reply"; }"#)
        .output()
        .expect("run noninteractive read prompt probe");

    assert_eq!(output.status.code(), Some(0));
    assert_eq!(String::from_utf8_lossy(&output.stdout), "<data>\n");
    assert_eq!(String::from_utf8_lossy(&output.stderr), "");
}

#[test]
fn assignment_shaped_command_argument_still_gets_pathname_expansion() {
    let output = Command::new(env!("CARGO_BIN_EXE_rubash"))
        .arg("-c")
        .arg(r#"rm -f RUBASH_GLOB_ASSIGN=one.rg one.rg; >RUBASH_GLOB_ASSIGN=one.rg; >one.rg; echo RUBASH_GLOB_ASSIGN=*.rg "RUBASH_GLOB_ASSIGN=*.rg"; rm -f RUBASH_GLOB_ASSIGN=one.rg one.rg"#)
        .output()
        .expect("run assignment-shaped glob argument probe");

    assert_eq!(output.status.code(), Some(0));
    assert_eq!(
        String::from_utf8_lossy(&output.stdout),
        "RUBASH_GLOB_ASSIGN=one.rg RUBASH_GLOB_ASSIGN=*.rg\n"
    );
    assert_eq!(String::from_utf8_lossy(&output.stderr), "");
}

#[test]
fn getopts_inline_option_argument_starts_after_option() {
    let output = Command::new(env!("CARGO_BIN_EXE_rubash"))
        .arg("-c")
        .arg(r#"set -- -w2; while getopts "w:" var; do printf '%s:%s:%s\n' "$var" "$OPTARG" "$OPTIND"; done"#)
        .output()
        .expect("run getopts inline option argument probe");

    assert_eq!(output.status.code(), Some(0));
    assert_eq!(String::from_utf8_lossy(&output.stdout), "w:2:2\n");
    assert_eq!(String::from_utf8_lossy(&output.stderr), "");
}

#[test]
fn getopts_invalid_option_diagnostic_has_bash_separator() {
    let output = Command::new(env!("CARGO_BIN_EXE_rubash"))
        .arg("-c")
        .arg(r#"while getopts "a" var -d; do :; done"#)
        .output()
        .expect("run getopts invalid option diagnostic probe");

    assert_eq!(output.status.code(), Some(0));
    assert_eq!(String::from_utf8_lossy(&output.stdout), "");
    assert!(String::from_utf8_lossy(&output.stderr).contains(": illegal option -- d"));
}

#[test]
fn assignment_shaped_argument_preserves_quoted_rhs_spaces() {
    let output = Command::new(env!("CARGO_BIN_EXE_rubash"))
        .arg("-c")
        .arg(r#"jv=16; let jv="$jv / 2"; printf '<%s>\n' jv="$jv / 2"; echo rc:$? jv:$jv"#)
        .output()
        .expect("run quoted assignment-shaped argument probe");

    assert_eq!(output.status.code(), Some(0));
    assert_eq!(String::from_utf8_lossy(&output.stderr), "");
    assert_eq!(
        String::from_utf8_lossy(&output.stdout),
        "<jv=8 / 2>\nrc:0 jv:8\n"
    );
}

#[test]
fn arithmetic_error_aborts_current_subshell_only() {
    let script = concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/tests/fixtures/arithmetic_error_subshell_continues_outer.sh"
    );
    let output = Command::new(env!("CARGO_BIN_EXE_rubash"))
        .arg(script)
        .output()
        .expect("run arithmetic subshell error probe");

    assert_eq!(output.status.code(), Some(0));
    assert_eq!(
        String::from_utf8_lossy(&output.stdout),
        "inner:9\nouter:9\n"
    );
    assert!(!String::from_utf8_lossy(&output.stderr).is_empty());
}

#[test]
fn arithmetic_logical_short_circuit_still_parses_bare_assignment() {
    let output = Command::new(env!("CARGO_BIN_EXE_rubash"))
        .arg("-c")
        .arg("B=9; echo $((0 && B=42)); echo after")
        .output()
        .expect("run arithmetic short-circuit assignment probe");

    assert_eq!(output.status.code(), Some(0));
    assert_eq!(String::from_utf8_lossy(&output.stdout), "after\n");
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

#[test]
fn quoted_parameter_pattern_glob_chars_are_literal() {
    let output = Command::new(env!("CARGO_BIN_EXE_rubash"))
        .arg("-c")
        .arg(r#"x='*'; printf '<%s>\n' "${x#'*'}"; x='a*b'; printf '<%s>\n' "${x#'a*'}""#)
        .output()
        .expect("run quoted parameter pattern glob probe");

    assert_eq!(output.status.code(), Some(0));
    assert_eq!(String::from_utf8_lossy(&output.stdout), "<>\n<b>\n");
    assert_eq!(String::from_utf8_lossy(&output.stderr), "");
}

#[test]
fn unquoted_heredoc_backslash_and_parameter_errors_match_bash() {
    let output = Command::new(env!("CARGO_BIN_EXE_rubash"))
        .arg("-c")
        .arg(concat!(
            "cat <<EOF\n",
            "a\\\n",
            "b\n",
            "c\\\\\n",
            "d\n",
            "EOF\n",
            "x='*'; printf '<%s>\\n' \"${x#'*'}\"\n",
            "M=ERR; cat <<EOF; printf 'status=%s\\n' \"$?\"\n",
            "${D?$M}\n",
            "EOF\n",
            "printf 'after\\n'\n",
        ))
        .output()
        .expect("run heredoc backslash and parameter error probe");

    assert_eq!(output.status.code(), Some(0));
    assert_eq!(
        String::from_utf8_lossy(&output.stdout),
        "ab\nc\\\nd\n<>\nstatus=1\nafter\n"
    );
    assert!(String::from_utf8_lossy(&output.stderr).contains("D: ERR"));
}

#[test]
fn heredoc_old_style_backticks_preserve_single_quoted_literals() {
    let output = Command::new(env!("CARGO_BIN_EXE_rubash"))
        .arg("-c")
        .arg(concat!(
            "a=qwerty\n",
            "cat <<EOF\n",
            "`echo '$a \\` \\*'`\n",
            "EOF\n",
        ))
        .output()
        .expect("run heredoc old-style backtick quote probe");

    assert_eq!(output.status.code(), Some(0));
    assert_eq!(String::from_utf8_lossy(&output.stdout), "$a ` \\*\n");
    assert_eq!(String::from_utf8_lossy(&output.stderr), "");
}

#[test]
fn malformed_heredoc_reports_offending_source_line() {
    let output = Command::new(env!("CARGO_BIN_EXE_rubash"))
        .arg("-c")
        .arg("<<EOF; then <W")
        .output()
        .expect("run malformed heredoc diagnostic probe");

    assert_eq!(output.status.code(), Some(2));
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(stderr.contains("syntax error near unexpected token `then'"));
    assert!(stderr.contains("`<<EOF; then <W'"));
}

#[test]
fn grouped_background_trap_receives_kill_from_parent() {
    let output = Command::new(env!("CARGO_BIN_EXE_rubash"))
        .arg("-c")
        .arg(r#"{ trap "echo got TERM" TERM; sleep 2; }& sleep 1; kill $!; wait; echo "Done: $?""#)
        .output()
        .expect("run grouped background trap probe");

    assert_eq!(output.status.code(), Some(0));
    assert_eq!(
        String::from_utf8_lossy(&output.stdout),
        "got TERM\nDone: 0\n"
    );
    assert_eq!(String::from_utf8_lossy(&output.stderr), "");
}

#[test]
fn external_pipeline_preserves_quoted_awk_field_separator_argument() {
    let output = Command::new(env!("CARGO_BIN_EXE_rubash"))
        .arg("-c")
        .arg(r#"printf 'a\tb\n' | awk -F '\t' 'BEGIN { print "FS=[" FS "]" }'"#)
        .output()
        .expect("run quoted awk field separator pipeline probe");

    assert!(output.status.success());
    assert_eq!(
        String::from_utf8_lossy(&output.stdout).replace("\r\n", "\n"),
        "FS=[\\t]\n"
    );
}

#[test]
fn unquoted_function_substitution_preserves_quoted_positional_arguments() {
    let output = Command::new(env!("CARGO_BIN_EXE_rubash"))
        .arg("-c")
        .arg("f(){ printf '%s\\n' \"$@\"; }; set -- a b; set -- $(f \"$@\"); printf '<%s>\\n' \"$@\"")
        .output()
        .expect("run function substitution positional probe");

    assert!(output.status.success());
    assert_eq!(String::from_utf8_lossy(&output.stdout), "<a>\n<b>\n");
    assert_eq!(String::from_utf8_lossy(&output.stderr), "");
}

#[test]
fn command_substitution_sed_restores_shell_sentinels() {
    let output = Command::new(env!("CARGO_BIN_EXE_rubash"))
        .arg("-c")
        .arg("v=5.3; a=`echo $v | sed 's:\\\\..*$::'`; b=$(echo $v | sed 's:^.*\\.::'); printf '%s:%s\\n' \"$a\" \"$b\"")
        .output()
        .expect("run command substitution sed sentinel probe");

    assert!(output.status.success());
    assert_eq!(String::from_utf8_lossy(&output.stdout), "5:3\n");
    assert_eq!(String::from_utf8_lossy(&output.stderr), "");
}

#[test]
fn mixed_backtick_assignment_uses_typed_fragments() {
    let output = Command::new(env!("CARGO_BIN_EXE_rubash"))
        .arg("-c")
        .arg("value=pre`printf x`post; printf '<%s>\\n' \"$value\"")
        .output()
        .expect("run mixed backtick assignment probe");

    assert!(output.status.success());
    assert_eq!(String::from_utf8_lossy(&output.stdout), "<prexpost>\n");
    assert_eq!(String::from_utf8_lossy(&output.stderr), "");
}

#[test]
fn mixed_dollar_and_backtick_assignment_uses_typed_fragments() {
    let output = Command::new(env!("CARGO_BIN_EXE_rubash"))
        .arg("-c")
        .arg("value=a$(printf b)`printf c`d; printf '<%s>\\n' \"$value\"")
        .output()
        .expect("run mixed dollar and backtick assignment probe");

    assert!(output.status.success());
    assert_eq!(String::from_utf8_lossy(&output.stdout), "<abcd>\n");
    assert_eq!(String::from_utf8_lossy(&output.stderr), "");
}

#[test]
fn quoted_mixed_assignment_preserves_substitution_context() {
    let output = Command::new(env!("CARGO_BIN_EXE_rubash"))
        .arg("-c")
        .arg("value=\"pre$(printf 'a b')`printf c`post\"; printf '<%s>\\n' \"$value\"")
        .output()
        .expect("run quoted mixed assignment probe");

    assert!(output.status.success());
    assert_eq!(String::from_utf8_lossy(&output.stdout), "<prea bcpost>\n");
    assert_eq!(String::from_utf8_lossy(&output.stderr), "");
}

#[test]
fn command_substitution_assignment_preserves_c0_payload_bytes() {
    let output = Command::new(env!("CARGO_BIN_EXE_rubash"))
        .arg("-c")
        .arg(r#"x=$(printf '\024'); printf '14 '; printf '%s' "$x" | od -An -tx1; x=$(printf '\025'); printf '15 '; printf '%s' "$x" | od -An -tx1; x=$(printf '\032'); printf '1a '; printf '%s' "$x" | od -An -tx1; x=$(printf '\037'); printf '1f '; printf '%s' "$x" | od -An -tx1"#)
        .output()
        .expect("run command substitution C0 payload probe");

    assert!(output.status.success());
    let normalized = String::from_utf8_lossy(&output.stdout)
        .split_whitespace()
        .collect::<Vec<_>>()
        .join(" ");
    assert_eq!(normalized, "14 14 15 15 1a 1a 1f 1f");
    assert_eq!(String::from_utf8_lossy(&output.stderr), "");
}

#[test]
fn backtick_command_substitution_preserves_raw_c0_variable_payload() {
    let output = Command::new(env!("CARGO_BIN_EXE_rubash"))
        .arg("-c")
        .arg(r#"x=$(printf '\024'); y=`printf '%s' "$x"`; printf '14 '; printf '%s' "$y" | od -An -tx1; x=$(printf '\025'); y=`printf '%s' "$x"`; printf '15 '; printf '%s' "$y" | od -An -tx1; x=$(printf '\032'); y=`printf '%s' "$x"`; printf '1a '; printf '%s' "$y" | od -An -tx1; x=$(printf '\037'); y=`printf '%s' "$x"`; printf '1f '; printf '%s' "$y" | od -An -tx1"#)
        .output()
        .expect("run backtick raw C0 payload probe");

    assert!(output.status.success());
    let normalized = String::from_utf8_lossy(&output.stdout)
        .split_whitespace()
        .collect::<Vec<_>>()
        .join(" ");
    assert_eq!(normalized, "14 14 15 15 1a 1a 1f 1f");
    assert_eq!(String::from_utf8_lossy(&output.stderr), "");
}
