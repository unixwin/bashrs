use std::io::Write;
use std::process::{Child, Command, Stdio};
use std::time::{Duration, Instant};
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
        .arg("coproc MY { printf 'coproc-ok\\n'; }; read -r out <&\"${MY[0]}\"; echo \"$out\"")
        .output()
        .expect("run rubash");

    assert!(output.status.success());
    assert_eq!(String::from_utf8_lossy(&output.stdout), "coproc-ok\n");
}

#[test]
fn c_command_keeps_unread_coproc_records_for_later_reads() {
    let output = Command::new(env!("CARGO_BIN_EXE_rubash"))
        .arg("-c")
        .arg(
            "coproc C { printf 'one\\ntwo\\n'; }; \
             read -r first <&${C[0]}; read -r second <&${C[0]}; \
             printf '%s:%s\\n' \"$first\" \"$second\"",
        )
        .output()
        .expect("run rubash");

    assert!(output.status.success());
    assert_eq!(String::from_utf8_lossy(&output.stdout), "one:two\n");
    assert_eq!(String::from_utf8_lossy(&output.stderr), "");
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
fn c_command_closing_coproc_stdin_fd_unblocks_reader() {
    let mut child = Command::new(env!("CARGO_BIN_EXE_rubash"))
        .arg("-c")
        .arg(
            "coproc C { cat; }; fd=${C[1]}; printf 'hi\\n' >&$fd; \
             eval \"exec ${fd}>&-\"; read -r x <&${C[0]}; \
             printf 'x=%s status=%s\\n' \"$x\" \"$?\"",
        )
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .expect("run rubash");

    assert!(wait_for_child_exit(&mut child, Duration::from_secs(3)));
    let output = child.wait_with_output().expect("wait rubash");

    assert!(output.status.success());
    assert_eq!(String::from_utf8_lossy(&output.stdout), "x=hi status=0\n");
    assert_eq!(String::from_utf8_lossy(&output.stderr), "");
}

#[test]
fn c_command_duplicates_coproc_stdin_fd_for_writes() {
    let mut child = Command::new(env!("CARGO_BIN_EXE_rubash"))
        .arg("-c")
        .arg(
            "coproc C { cat; }; exec 7>&${C[1]}; printf 'hi\\n' >&7; \
             eval \"exec ${C[1]}>&-\"; exec 7>&-; read -r x <&${C[0]}; \
             printf 'x=%s status=%s\\n' \"$x\" \"$?\"",
        )
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .expect("run rubash");

    assert!(wait_for_child_exit(&mut child, Duration::from_secs(3)));
    let output = child.wait_with_output().expect("wait rubash");

    assert!(output.status.success());
    assert_eq!(String::from_utf8_lossy(&output.stdout), "x=hi status=0\n");
    assert_eq!(String::from_utf8_lossy(&output.stderr), "");
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
fn pipeline_statuses_match_bash_for_pipefail() {
    let output = Command::new(env!("CARGO_BIN_EXE_rubash"))
        .arg("-c")
        .arg("set -o pipefail; false | true; printf 'rc=%s ps=%s len=%s\\n' \"$?\" \"${PIPESTATUS[*]}\" \"${#PIPESTATUS[@]}\"")
        .output()
        .expect("run rubash");

    assert!(output.status.success());
    assert_eq!(
        String::from_utf8_lossy(&output.stdout),
        "rc=1 ps=1 0 len=2\n"
    );
}

#[test]
fn stderr_pipeline_preserves_output_and_statuses() {
    let output = Command::new(env!("CARGO_BIN_EXE_rubash"))
        .arg("-c")
        .arg("printf 'pipe-stderr\\n' |& cat; printf 'ps=%s len=%s\\n' \"${PIPESTATUS[*]}\" \"${#PIPESTATUS[@]}\"")
        .output()
        .expect("run rubash");

    assert!(output.status.success());
    assert_eq!(
        String::from_utf8_lossy(&output.stdout),
        "pipe-stderr\nps=0 0 len=2\n"
    );
}

#[test]
fn heredoc_removes_unquoted_backslash_newline_like_bash() {
    let output = Command::new(env!("CARGO_BIN_EXE_rubash"))
        .arg("-c")
        .arg("cat <<EOF\none\\\ntwo\nEOF")
        .output()
        .expect("run rubash");

    assert!(output.status.success());
    assert_eq!(String::from_utf8_lossy(&output.stdout), "onetwo\n");
}

#[test]
fn quoted_heredoc_preserves_backslash_newline() {
    let output = Command::new(env!("CARGO_BIN_EXE_rubash"))
        .arg("-c")
        .arg("cat <<'EOF'\none\\\ntwo\nEOF")
        .output()
        .expect("run rubash");

    assert!(output.status.success());
    assert_eq!(String::from_utf8_lossy(&output.stdout), "one\\\ntwo\n");
}

#[cfg(windows)]
#[test]
fn quoted_environment_paths_keep_native_windows_form() {
    let expected = std::env::var("LOCALAPPDATA").expect("LOCALAPPDATA is set");
    let output = Command::new(env!("CARGO_BIN_EXE_rubash"))
        .arg("-c")
        .arg("printf '%s\\n' \"${LOCALAPPDATA}\"")
        .output()
        .expect("run rubash");

    assert!(output.status.success());
    assert_eq!(
        String::from_utf8_lossy(&output.stdout),
        format!("{expected}\n")
    );
}

#[test]
fn nested_parameter_expansion_can_supply_pattern_removal() {
    let output = Command::new(env!("CARGO_BIN_EXE_rubash"))
        .arg("-c")
        .arg(
            "v=a\n echo \"hash=${v#?}\"\n echo \"pct=${v%\"${v#?}\"}\"\n \
             v=ab\n echo \"hash2=${v#?}\"\n echo \"pct2=${v%\"${v#?}\"}\"",
        )
        .output()
        .expect("run rubash");

    assert!(output.status.success());
    assert_eq!(
        String::from_utf8_lossy(&output.stdout),
        "hash=\npct=a\nhash2=b\npct2=a\n"
    );
}

#[test]
fn arithmetic_errors_in_assignment_abort_the_script() {
    for (prefix, assignment) in [
        ("", "x=$((1.5))"),
        ("", "x=$(( '1' ))"),
        ("set -u;", "x=$((missing))"),
    ] {
        let output = Command::new(env!("CARGO_BIN_EXE_rubash"))
            .arg("-c")
            .arg(format!("{prefix}{assignment}; echo should-not-run"))
            .output()
            .expect("run rubash");

        assert_eq!(
            output.status.code(),
            Some(1),
            "assignment: {prefix}{assignment}"
        );
        assert_eq!(String::from_utf8_lossy(&output.stdout), "");
        assert!(!String::from_utf8_lossy(&output.stderr).is_empty());
    }
}

#[test]
fn arithmetic_expansion_error_only_fails_the_current_command() {
    let output = Command::new(env!("CARGO_BIN_EXE_rubash"))
        .arg("-c")
        .arg("echo $((2#44)); echo after")
        .output()
        .expect("run rubash");

    assert!(output.status.success());
    assert_eq!(String::from_utf8_lossy(&output.stdout), "after\n");
    assert!(String::from_utf8_lossy(&output.stderr).contains("value too great for base"));
}

#[test]
fn arithmetic_commands_reject_single_quoted_operands() {
    let output = Command::new(env!("CARGO_BIN_EXE_rubash"))
        .arg("-c")
        .arg("(( '1' )); echo after")
        .output()
        .expect("run rubash");

    assert!(output.status.success());
    assert_eq!(String::from_utf8_lossy(&output.stdout), "after\n");
    assert!(String::from_utf8_lossy(&output.stderr).contains("operand expected"));
}

#[test]
fn function_call_stack_reports_multiline_source_and_line() {
    let output = Command::new(env!("CARGO_BIN_EXE_rubash"))
        .arg("-c")
        .arg(
            "t2() { printf '%s|%s|%s\\n' \"${FUNCNAME[*]}\" \"${BASH_SOURCE[*]}\" \"${BASH_LINENO[*]}\"; }\nt2\n",
        )
        .output()
        .expect("run rubash");

    assert!(output.status.success());
    assert_eq!(
        String::from_utf8_lossy(&output.stdout),
        "t2 main|main|2 0\n"
    );
}

#[test]
fn umask_symbolic_mode_prints_after_setting_mode() {
    let output = Command::new(env!("CARGO_BIN_EXE_rubash"))
        .arg("-c")
        .arg("umask -S 0002")
        .output()
        .expect("run rubash");

    assert!(output.status.success());
    assert_eq!(
        String::from_utf8_lossy(&output.stdout),
        "u=rwx,g=rwx,o=rx\n"
    );
}

#[test]
fn malformed_pipeline_and_if_are_syntax_errors() {
    for command in [
        "echo hi |",
        "echo hi &&",
        "echo hi & && echo x",
        "if then; fi; echo after",
        "<<EOF; then <W",
        "while; do :; done",
        "case x in x) ;;",
        "( echo hi",
        "{ echo hi;",
        "echo @(",
        "echo !(",
        "echo +(",
    ] {
        let output = Command::new(env!("CARGO_BIN_EXE_rubash"))
            .arg("-c")
            .arg(command)
            .output()
            .expect("run rubash");
        assert!(
            !output.status.success(),
            "command unexpectedly succeeded: {command}"
        );
        assert!(String::from_utf8_lossy(&output.stderr).contains("syntax error"));
    }
}

#[test]
fn malformed_case_reserved_word_boundaries_are_syntax_errors() {
    for command in [
        "case x in esac) echo done; esac",
        "case in do do) echo in; esac",
    ] {
        let output = Command::new(env!("CARGO_BIN_EXE_rubash"))
            .arg("-c")
            .arg(command)
            .output()
            .expect("run rubash");
        assert_eq!(output.status.code(), Some(2), "command: {command}");
        assert!(String::from_utf8_lossy(&output.stderr).contains("syntax error"));
    }
}

#[test]
fn newline_for_header_inside_case_is_a_syntax_error() {
    let output = Command::new(env!("CARGO_BIN_EXE_rubash"))
        .arg("-c")
        .arg("case x in x)\nfor x\nin x\ndo echo bad; done\nesac")
        .output()
        .expect("run rubash");

    assert_eq!(output.status.code(), Some(2));
    assert!(String::from_utf8_lossy(&output.stdout).is_empty());
    assert!(
        String::from_utf8_lossy(&output.stderr).contains("syntax error near unexpected token `do'")
    );
}

#[test]
fn invalid_for_and_select_names_fail_at_execution_like_bash() {
    for command in [
        "for invalid-name in a b; do echo bad; done",
        "for 1 in a b; do echo bad; done",
        "select invalid-name in a b; do echo bad; done",
    ] {
        let output = Command::new(env!("CARGO_BIN_EXE_rubash"))
            .arg("-c")
            .arg(command)
            .output()
            .expect("run rubash");
        assert_eq!(output.status.code(), Some(1), "command: {command}");
        assert!(String::from_utf8_lossy(&output.stderr).contains("not a valid identifier"));
        assert!(!String::from_utf8_lossy(&output.stdout).contains("bad"));
    }
}

#[test]
fn invalid_for_name_is_fatal_in_posix_mode() {
    let output = Command::new(env!("CARGO_BIN_EXE_rubash"))
        .arg("-c")
        .arg("set -o posix; for invalid-name in a b; do echo bad; done; echo after")
        .output()
        .expect("run rubash");

    assert_eq!(output.status.code(), Some(2));
    assert_eq!(String::from_utf8_lossy(&output.stdout), "");
    assert!(String::from_utf8_lossy(&output.stderr).contains("not a valid identifier"));
}

#[test]
fn malformed_conditional_operator_is_a_syntax_error() {
    for command in ["[[ -n & ]]", "[[ 4 & ]]", "[[ -n < ]]"] {
        let output = Command::new(env!("CARGO_BIN_EXE_rubash"))
            .arg("-c")
            .arg(command)
            .output()
            .expect("run rubash");
        assert_eq!(output.status.code(), Some(2), "command: {command}");
    }
}

#[test]
fn unterminated_conditional_is_a_syntax_error() {
    for command in ["[[ -n foo", "[[ ( -t X ) ]", "[[ -n foo ]"] {
        let output = Command::new(env!("CARGO_BIN_EXE_rubash"))
            .arg("-c")
            .arg(command)
            .output()
            .expect("run rubash");
        assert_eq!(output.status.code(), Some(2), "command: {command}");
    }
}

#[test]
fn unterminated_command_substitution_is_a_syntax_error() {
    let output = Command::new(env!("CARGO_BIN_EXE_rubash"))
        .arg("-c")
        .arg("echo $(printf foo")
        .output()
        .expect("run rubash");
    assert_eq!(output.status.code(), Some(2));
}

#[test]
fn command_substitution_still_executes_when_closed() {
    let output = Command::new(env!("CARGO_BIN_EXE_rubash"))
        .arg("-c")
        .arg("echo \"$(printf '%s' ok)\"")
        .output()
        .expect("run rubash");
    assert!(output.status.success());
    assert_eq!(String::from_utf8_lossy(&output.stdout), "ok\n");
}

#[test]
fn malformed_compound_inside_command_substitution_is_a_syntax_error() {
    let output = Command::new(env!("CARGO_BIN_EXE_rubash"))
        .arg("-c")
        .arg("echo $( if x; then echo foo )")
        .output()
        .expect("run rubash");
    assert_eq!(output.status.code(), Some(2));
}

#[test]
fn arithmetic_for_errors_preserve_failure_status() {
    for (command, expected_stdout) in [
        (
            "for (( 7=1; i<4; i++ )); do echo body; done; echo status:$?",
            "status:1\n",
        ),
        (
            "for (( i=1; 7++; i++ )); do echo body; done; echo status:$?",
            "status:1\n",
        ),
        (
            "for (( i=1; i<4; 7++ )); do echo body; done; echo status:$?",
            "body\nstatus:1\n",
        ),
    ] {
        let output = Command::new(env!("CARGO_BIN_EXE_rubash"))
            .arg("-c")
            .arg(command)
            .output()
            .expect("run rubash");
        assert!(
            output.status.success(),
            "script should reach status echo: {command}"
        );
        assert_eq!(
            String::from_utf8_lossy(&output.stdout),
            expected_stdout,
            "command: {command}"
        );
        assert!(
            !output.stderr.is_empty(),
            "missing arithmetic diagnostic: {command}"
        );
    }
}

#[test]
fn arithmetic_builtin_errors_report_their_owner() {
    for (command, marker) in [
        ("let", "let: expression expected"),
        ("let '4 +'", "let: 4 +: syntax error: operand expected"),
        (
            "let '7=4'",
            "let: 7=4: attempted assignment to non-variable",
        ),
        ("[[ 1/0 -eq 1 ]]", "[[: 1/0: division by 0"),
    ] {
        let output = Command::new(env!("CARGO_BIN_EXE_rubash"))
            .arg("-c")
            .arg(command)
            .output()
            .expect("run rubash");
        assert_eq!(output.status.code(), Some(1), "command: {command}");
        assert!(
            String::from_utf8_lossy(&output.stderr).contains(marker),
            "command: {command}, stderr: {}",
            String::from_utf8_lossy(&output.stderr)
        );
    }
}

#[test]
fn arithmetic_expansion_preserves_non_lvalue_increment_operand() {
    let output = Command::new(env!("CARGO_BIN_EXE_rubash"))
        .arg("-c")
        .arg("echo $((--7)); echo $((++ 7)); (( -- ))")
        .output()
        .expect("run rubash");
    assert_eq!(String::from_utf8_lossy(&output.stdout), "7\n7\n");
    assert_eq!(output.status.code(), Some(1));
    assert!(String::from_utf8_lossy(&output.stderr)
        .contains("((: -- : syntax error: operand expected (error token is \"- \")"));
}

#[test]
fn empty_arithmetic_contexts_match_bash_zero_semantics() {
    let output = Command::new(env!("CARGO_BIN_EXE_rubash"))
        .arg("-c")
        .arg("echo A:$(( )); echo B:$(( \"\" )); (( )); echo command:$?; [[ 0 -eq \"\" ]]; echo cond:$?")
        .output()
        .expect("run rubash");
    assert_eq!(output.status.code(), Some(0));
    assert_eq!(
        String::from_utf8_lossy(&output.stdout),
        "A:0\nB:0\ncommand:1\ncond:0\n"
    );
}

#[test]
fn arithmetic_array_subscript_quote_removal_targets_index_zero() {
    let output = Command::new(env!("CARGO_BIN_EXE_rubash"))
        .arg("-c")
        .arg("declare -a a; a[0]=0; (( a[\" \"]=11 )); declare -p a")
        .output()
        .expect("run rubash");
    assert!(output.status.success());
    assert_eq!(
        String::from_utf8_lossy(&output.stdout),
        "declare -a a=([0]=\"11\")\n"
    );
}

#[test]
fn array_element_assignment_reports_bash_subscript_errors() {
    let output = Command::new(env!("CARGO_BIN_EXE_rubash"))
        .arg("-c")
        .arg(
            "unset b c d; b[]=bcde; printf 'b=%s\\n' \"$?\"; \
             b[*]=aaa; printf 'star=%s\\n' \"$?\"; \
             c[-2]=4; printf 'negative=%s\\n' \"$?\"; \
             d[7]=(x y); printf 'list=%s\\n' \"$?\"",
        )
        .output()
        .expect("run rubash");

    assert_eq!(output.status.code(), Some(0));
    assert_eq!(
        String::from_utf8_lossy(&output.stdout),
        "b=1\nstar=1\nnegative=1\nlist=1\n"
    );
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(stderr.contains("b[]=bcde: bad array subscript"));
    assert!(stderr.contains("b[*]=aaa: cannot assign to non-numeric index"));
    assert!(stderr.contains("c[-2]=4: bad array subscript"));
    assert!(stderr.contains("d[7]=(x y): cannot assign list to array member"));
}

#[test]
fn readonly_array_element_argument_matches_bash_identifier_diagnostic() {
    let output = Command::new(env!("CARGO_BIN_EXE_rubash"))
        .arg("-c")
        .arg("unset a; a=(zero one); readonly a[1]")
        .output()
        .expect("run rubash");

    assert_eq!(output.status.code(), Some(1));
    assert!(String::from_utf8_lossy(&output.stderr)
        .contains("readonly: `a[1]`: not a valid identifier"));
}

#[test]
fn compound_inside_command_substitution_still_executes_when_closed() {
    let output = Command::new(env!("CARGO_BIN_EXE_rubash"))
        .arg("-c")
        .arg("echo $( if true; then echo foo; fi )")
        .output()
        .expect("run rubash");
    assert!(output.status.success());
    assert_eq!(String::from_utf8_lossy(&output.stdout), "foo\n");
}

#[test]
fn invalid_case_terminator_inside_command_substitution_is_a_syntax_error() {
    let output = Command::new(env!("CARGO_BIN_EXE_rubash"))
        .arg("-c")
        .arg("echo $(case x in x) ;; x) done ;; esac)")
        .output()
        .expect("run rubash");
    assert_eq!(output.status.code(), Some(2));
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
fn external_pipeline_waits_for_all_members_of_a_multi_stage_pipeline() {
    let output = Command::new(env!("CARGO_BIN_EXE_rubash"))
        .arg("-c")
        .arg("yes pipeline | head -n 3 | wc -l")
        .output()
        .expect("run rubash");

    assert!(output.status.success());
    assert_eq!(String::from_utf8_lossy(&output.stdout), "3\n");
}

#[test]
#[cfg(windows)]
fn external_pipeline_writes_large_output_redirect_without_blocking() {
    let output_path = std::env::temp_dir().join("rubash-large-pipeline-output.txt");
    let _ = fs::remove_file(&output_path);
    let command = format!(
        "yes '123456789 123456789 123456789 123456789' | head -3000 >> '{}'",
        shell_test_path(&output_path)
    );
    let output = Command::new(env!("CARGO_BIN_EXE_rubash"))
        .arg("-c")
        .arg(command)
        .output()
        .expect("run rubash");

    let contents = fs::read(&output_path).expect("read pipeline output");
    let _ = fs::remove_file(&output_path);
    assert!(output.status.success());
    assert_eq!(contents.len(), 120_000);
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
fn external_pipeline_preserves_limited_head_output() {
    let output = Command::new(env!("CARGO_BIN_EXE_rubash"))
        .arg("-c")
        .arg("printf 'pipeline\\npipeline\\npipeline\\nextra\\n' | head -n 3")
        .output()
        .expect("run rubash");

    assert!(output.status.success());
    assert_eq!(
        String::from_utf8_lossy(&output.stdout),
        "pipeline\npipeline\npipeline\n"
    );
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
fn c_command_echo_applies_output_redirects_left_to_right() {
    let literal_fd_path = Path::new("&3");
    let _ = fs::remove_file(literal_fd_path);

    let output = Command::new(env!("CARGO_BIN_EXE_rubash"))
        .arg("-c")
        .arg("echo hi 3>&1 1>/dev/null >&3; printf 'status:%s\\n' \"$?\"")
        .output()
        .expect("run rubash");

    assert!(output.status.success());
    assert_eq!(String::from_utf8_lossy(&output.stdout), "hi\nstatus:0\n");
    assert_eq!(String::from_utf8_lossy(&output.stderr), "");
    assert!(!literal_fd_path.exists());
}

#[test]
fn c_command_echo_reports_bad_fd_after_exec_close() {
    let literal_fd_path = Path::new("&3");
    let _ = fs::remove_file(literal_fd_path);

    let output = Command::new(env!("CARGO_BIN_EXE_rubash"))
        .arg("-c")
        .arg("exec 3>&1; echo hi >&3; exec 3>&-; echo fail >&3; printf 'status:%s\\n' \"$?\"")
        .output()
        .expect("run rubash");

    assert!(output.status.success());
    assert_eq!(String::from_utf8_lossy(&output.stdout), "hi\nstatus:1\n");
    assert_eq!(
        String::from_utf8_lossy(&output.stderr),
        "rubash: 3: Bad file descriptor\n"
    );
    assert!(!literal_fd_path.exists());
}

#[test]
fn c_command_echo_reports_write_error_for_closed_stdout() {
    let output = Command::new(env!("CARGO_BIN_EXE_rubash"))
        .arg("-c")
        .arg("echo fail >&-; printf 'status:%s\\n' \"$?\"")
        .output()
        .expect("run rubash");

    assert!(output.status.success());
    assert_eq!(String::from_utf8_lossy(&output.stdout), "status:1\n");
    assert_eq!(
        String::from_utf8_lossy(&output.stderr),
        "rubash: echo: write error: Bad file descriptor\n"
    );
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
fn c_command_read_dev_null_reports_eof() {
    let output = Command::new(env!("CARGO_BIN_EXE_rubash"))
        .arg("-c")
        .arg("read value < /dev/null; printf '<%s>:%s\\n' \"$value\" \"$?\"")
        .output()
        .expect("run rubash");

    assert!(output.status.success());
    assert_eq!(String::from_utf8_lossy(&output.stdout), "<>:1\n");
    assert_eq!(String::from_utf8_lossy(&output.stderr), "");
}

#[test]
fn c_command_redirects_stdout_and_stderr_to_dev_null() {
    let output = Command::new(env!("CARGO_BIN_EXE_rubash"))
        .arg("-c")
        .arg("printf out >/dev/null; command-that-does-not-exist 2>/dev/null; printf ok")
        .output()
        .expect("run rubash");

    assert!(output.status.success());
    assert_eq!(String::from_utf8_lossy(&output.stdout), "ok");
    assert_eq!(String::from_utf8_lossy(&output.stderr), "");
}

#[test]
fn c_external_command_reads_eof_from_null_device_alias() {
    let output = Command::new(env!("CARGO_BIN_EXE_rubash"))
        .arg("-c")
        .arg("cat < nUl; printf 'status:%s\\n' \"$?\"")
        .output()
        .expect("run rubash");

    assert!(output.status.success());
    assert_eq!(String::from_utf8_lossy(&output.stdout), "status:0\n");
    assert_eq!(String::from_utf8_lossy(&output.stderr), "");
}

#[test]
fn c_command_appends_stdout_to_null_device_without_creating_path() {
    let output = Command::new(env!("CARGO_BIN_EXE_rubash"))
        .arg("-c")
        .arg("printf first >> /dev/null; printf second >> /dev/null; printf ok")
        .output()
        .expect("run rubash");

    assert!(output.status.success());
    assert_eq!(String::from_utf8_lossy(&output.stdout), "ok");
    assert_eq!(String::from_utf8_lossy(&output.stderr), "");
}

#[test]
fn c_command_read_closed_stdin_reports_bad_fd() {
    let output = Command::new(env!("CARGO_BIN_EXE_rubash"))
        .arg("-c")
        .arg("read value <&-; printf '<%s>:%s\\n' \"$value\" \"$?\"")
        .output()
        .expect("run rubash");

    assert!(output.status.success());
    assert_eq!(String::from_utf8_lossy(&output.stdout), "<>:1\n");
    assert_eq!(
        String::from_utf8_lossy(&output.stderr),
        "rubash: read: read error: 0: Bad file descriptor\n"
    );
}

#[test]
fn c_command_read_closed_stdin_redirects_bad_fd_diagnostic() {
    let error_path = Path::new("target").join("rubash-cli-read-closed-stderr.txt");
    let _ = fs::remove_file(&error_path);
    let script_path = shell_test_path(&error_path);

    let output = Command::new(env!("CARGO_BIN_EXE_rubash"))
        .arg("-c")
        .arg(format!(
            "read value <&- 2> {script_path}; printf '<%s>:%s\\n' \"$value\" \"$?\""
        ))
        .output()
        .expect("run rubash");

    assert!(output.status.success());
    assert_eq!(String::from_utf8_lossy(&output.stdout), "<>:1\n");
    assert_eq!(String::from_utf8_lossy(&output.stderr), "");
    assert_eq!(
        fs::read_to_string(&error_path)
            .unwrap()
            .replace("\r\n", "\n"),
        "rubash: read: read error: 0: Bad file descriptor\n"
    );
    let _ = fs::remove_file(error_path);
}

#[test]
fn c_command_read_uses_regular_stdin_redirect_file() {
    let input_path = Path::new("target").join("rubash-cli-read-redirect-input.txt");
    fs::write(&input_path, "from-file\n").unwrap();
    let script_path = shell_test_path(&input_path);

    let output = Command::new(env!("CARGO_BIN_EXE_rubash"))
        .arg("-c")
        .arg(format!(
            "read value < {script_path}; printf '<%s>:%s\\n' \"$value\" \"$?\""
        ))
        .output()
        .expect("run rubash");

    assert!(output.status.success());
    assert_eq!(String::from_utf8_lossy(&output.stdout), "<from-file>:0\n");
    assert_eq!(String::from_utf8_lossy(&output.stderr), "");
    let _ = fs::remove_file(input_path);
}

#[test]
fn c_command_kill_zero_accepts_current_process() {
    let output = Command::new(env!("CARGO_BIN_EXE_rubash"))
        .arg("-c")
        .arg(format!("kill -0 {}", std::process::id()))
        .output()
        .expect("run rubash");

    assert!(
        output.status.success(),
        "stderr: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    assert_eq!(String::from_utf8_lossy(&output.stderr), "");
}

#[test]
fn c_command_kill_zero_accepts_shell_pid() {
    let output = Command::new(env!("CARGO_BIN_EXE_rubash"))
        .arg("-c")
        .arg("kill -0 $$; printf 'status:%s\\n' \"$?\"")
        .output()
        .expect("run rubash");

    assert!(output.status.success());
    assert_eq!(String::from_utf8_lossy(&output.stdout), "status:0\n");
    assert_eq!(String::from_utf8_lossy(&output.stderr), "");
}

#[test]
fn c_command_kill_zero_accepts_current_process_group() {
    let output = Command::new(env!("CARGO_BIN_EXE_rubash"))
        .arg("-c")
        .arg("kill -0 0; printf 'status:%s\\n' \"$?\"")
        .output()
        .expect("run rubash");

    assert!(output.status.success());
    assert_eq!(String::from_utf8_lossy(&output.stdout), "status:0\n");
    assert_eq!(String::from_utf8_lossy(&output.stderr), "");
}

#[test]
fn c_command_background_subshell_preserves_shell_pid() {
    let output = Command::new(env!("CARGO_BIN_EXE_rubash"))
        .arg("-c")
        .arg("echo parent:$$:$BASHPID; (echo bg:$$:$BASHPID) & wait")
        .output()
        .expect("run rubash");

    assert!(
        output.status.success(),
        "stderr: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    let stdout = String::from_utf8_lossy(&output.stdout);
    let mut lines = stdout.lines();
    let parent = lines.next().expect("parent line");
    let child = lines.next().expect("child line");
    let parent_fields = parent.split(':').collect::<Vec<_>>();
    let child_fields = child.split(':').collect::<Vec<_>>();
    assert_eq!(parent_fields[1], child_fields[1], "$$ should stay stable");
    assert_ne!(
        parent_fields[2], child_fields[2],
        "BASHPID should identify the background child"
    );
}

#[test]
fn c_command_background_kill_shell_pid_runs_term_trap() {
    let delay = if cfg!(windows) {
        "powershell.exe -NoProfile -NonInteractive -ExecutionPolicy Bypass -Command Start-Sleep -Milliseconds 250; "
    } else {
        "sleep 0.25; "
    };
    let script = format!(
        "trap 'echo TERM; return' TERM; \
         f() {{ ({delay}kill $$) & until (exit 42); do (exit 42); done; }}; \
         f; printf 'status:%s\\n' \"$?\""
    );
    let output = Command::new(env!("CARGO_BIN_EXE_rubash"))
        .arg("-c")
        .arg(script)
        .output()
        .expect("run rubash");

    assert!(
        output.status.success(),
        "stderr: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    assert_eq!(String::from_utf8_lossy(&output.stdout), "TERM\nstatus:42\n");
}

#[test]
fn c_command_kill_rejects_invalid_pid_operand() {
    let output = Command::new(env!("CARGO_BIN_EXE_rubash"))
        .arg("-c")
        .arg("kill abc")
        .output()
        .expect("run rubash");

    assert!(!output.status.success());
    assert!(String::from_utf8_lossy(&output.stderr)
        .contains("kill: abc: arguments must be process or job IDs"));
}

#[test]
fn c_command_kill_sigkill_terminates_windows_pid() {
    let mut child = spawn_long_child();
    let pid = child.id();

    let output = Command::new(env!("CARGO_BIN_EXE_rubash"))
        .arg("-c")
        .arg(format!("kill -9 {pid}"))
        .output()
        .expect("run rubash");

    assert!(
        output.status.success(),
        "stderr: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    assert!(
        wait_for_child_exit(&mut child, Duration::from_secs(5)),
        "child process {pid} did not exit after kill -9"
    );
}

#[test]
fn c_command_kill_sigkill_terminates_rubash_child_pid() {
    let mut child = Command::new(env!("CARGO_BIN_EXE_rubash"))
        .arg("-c")
        .arg("while :; do :; done")
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .spawn()
        .expect("spawn long-running rubash child");
    let pid = child.id();

    let output = Command::new(env!("CARGO_BIN_EXE_rubash"))
        .arg("-c")
        .arg(format!("kill -9 {pid}"))
        .output()
        .expect("run rubash");

    assert!(
        output.status.success(),
        "stderr: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    assert!(
        wait_for_child_exit(&mut child, Duration::from_secs(5)),
        "rubash child process {pid} did not exit after kill -9"
    );
}

#[cfg(windows)]
fn spawn_long_child() -> Child {
    Command::new("powershell.exe")
        .args([
            "-NoProfile",
            "-NonInteractive",
            "-ExecutionPolicy",
            "Bypass",
            "-Command",
            "Start-Sleep -Seconds 30",
        ])
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .spawn()
        .expect("spawn long-running child")
}

#[cfg(not(windows))]
fn spawn_long_child() -> Child {
    Command::new("sleep")
        .arg("30")
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .spawn()
        .expect("spawn long-running child")
}

fn wait_for_child_exit(child: &mut Child, timeout: Duration) -> bool {
    let start = Instant::now();
    while start.elapsed() < timeout {
        if child.try_wait().expect("poll child").is_some() {
            return true;
        }
        std::thread::sleep(Duration::from_millis(50));
    }

    let _ = child.kill();
    let _ = child.wait();
    false
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
fn c_command_mapfile_reports_bad_fd_after_exec_close() {
    let output = Command::new(env!("CARGO_BIN_EXE_rubash"))
        .arg("-c")
        .arg("exec 3<&-; mapfile -u3 arr; printf 'status:%s len:%s\\n' \"$?\" \"${#arr[@]}\"")
        .output()
        .expect("run rubash");

    assert!(output.status.success());
    assert_eq!(String::from_utf8_lossy(&output.stdout), "status:1 len:0\n");
    assert_eq!(
        String::from_utf8_lossy(&output.stderr),
        "rubash: mapfile: 3: invalid file descriptor: Bad file descriptor\n"
    );
}

#[test]
fn c_command_mapfile_u_accepts_expanded_persistent_fd() {
    let input_path = "target/rubash-cli-mapfile-fd-input.txt";
    let _ = fs::remove_file(input_path);
    fs::write(input_path, "alpha\nbeta\n").expect("write mapfile input");

    let output = Command::new(env!("CARGO_BIN_EXE_rubash"))
        .arg("-c")
        .arg(format!(
            "exec 3<{input_path}; mapfile -u \"$((1+2))\" -t arr; \
             printf '%s:%s:%s:%s\\n' \"$?\" \"${{#arr[@]}}\" \"${{arr[0]}}\" \"${{arr[1]}}\""
        ))
        .output()
        .expect("run rubash");

    assert!(output.status.success());
    assert_eq!(String::from_utf8_lossy(&output.stdout), "0:2:alpha:beta\n");
    assert_eq!(String::from_utf8_lossy(&output.stderr), "");

    let _ = fs::remove_file(input_path);
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

#[test]
fn c_command_set_o_invalid_name_returns_usage_status() {
    let output = Command::new(env!("CARGO_BIN_EXE_rubash"))
        .arg("-c")
        .arg("set -o no_such; printf 'status:%s\\n' \"$?\"")
        .output()
        .expect("run rubash");

    assert!(output.status.success());
    assert_eq!(String::from_utf8_lossy(&output.stdout), "status:2\n");
    assert!(String::from_utf8_lossy(&output.stderr).contains("invalid option name"));
}
