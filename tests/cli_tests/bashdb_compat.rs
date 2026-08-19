use std::io::Write;
use std::process::{Command, Stdio};
use std::{fs, path::Path};

fn run_rubash_inline(script: &str) -> std::process::Output {
    Command::new(env!("CARGO_BIN_EXE_rubash"))
        .arg("-c")
        .arg(script)
        .output()
        .expect("run rubash")
}

#[test]
fn unquoted_embedded_parameter_expansion_splits_fields() {
    let output = run_rubash_inline(
        "opts='hBT -o functrace'\nset -$opts\nprintf 'status=%s flags=%s\\n' \"$?\" \"$-\"\n",
    );

    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(output.status.success());
    assert!(stdout.starts_with("status=0 flags="));
    assert!(stdout.contains('T'));
    assert_eq!(String::from_utf8_lossy(&output.stderr), "");
}

#[test]
fn for_loop_quoted_positional_at_iterates_each_argument() {
    let output = run_rubash_inline(
        "set -- --no-highlight target/bashdb-probe-target.sh\nfor arg in \"$@\"; do printf '[%s]\\n' \"$arg\"; done\n",
    );

    assert!(output.status.success());
    assert_eq!(
        String::from_utf8_lossy(&output.stdout),
        "[--no-highlight]\n[target/bashdb-probe-target.sh]\n"
    );
    assert_eq!(String::from_utf8_lossy(&output.stderr), "");
}

#[test]
fn arithmetic_command_uses_full_expression_with_positional_count() {
    let output = run_rubash_inline(
        "set -- x\nif (($# == 0)); then echo zero; else echo one; fi\nset --\nif (($# == 0)); then echo zero; else echo one; fi\n",
    );

    assert!(output.status.success());
    assert_eq!(String::from_utf8_lossy(&output.stdout), "one\nzero\n");
    assert_eq!(String::from_utf8_lossy(&output.stderr), "");
}

#[test]
fn multiline_arithmetic_for_header_allows_line_breaks() {
    let output =
        run_rubash_inline("for (( i=0;\n       (( i < 2 )) ;\n       i++ )) ; do echo $i; done\n");

    assert!(output.status.success());
    assert_eq!(String::from_utf8_lossy(&output.stdout), "0\n1\n");
    assert_eq!(String::from_utf8_lossy(&output.stderr), "");
}

#[test]
fn kill_stdout_is_captured_when_stderr_is_redirected_in_function_substitution() {
    let output = run_rubash_inline(
        "f(){ builtin kill -l ILL 2>/dev/null; return $?; }\ntypeset -i x=$(f)\nprintf 'x=%s\\n' \"$x\"\n",
    );

    assert!(output.status.success());
    assert_eq!(String::from_utf8_lossy(&output.stdout), "x=4\n");
    assert_eq!(String::from_utf8_lossy(&output.stderr), "");
}

#[test]
fn eval_reparse_preserves_escaped_backticks_inside_double_quotes() {
    let output = run_rubash_inline(
        r#"f() {
  set -- x optname
  LC_ALL=C command eval '
    case "$2" in
      *[!a-zA-Z_0-9]*|""|[0-9]*)
        printf >&2 "getopts_long: invalid variable name: \`%s'\''\n" "$2"
        return 1
        ;;
    esac'
  echo fafter
}
f
echo after
"#,
    );

    assert!(output.status.success());
    assert_eq!(String::from_utf8_lossy(&output.stdout), "fafter\nafter\n");
    assert_eq!(String::from_utf8_lossy(&output.stderr), "");
}

#[test]
fn command_enable_a_with_missing_operand_reports_failure() {
    let output = run_rubash_inline("enable -a set0\nprintf 'status=%s\\n' \"$?\"\n");

    assert!(output.status.success());
    assert_eq!(String::from_utf8_lossy(&output.stdout), "status=1\n");
    assert!(String::from_utf8_lossy(&output.stderr).contains("set0"));
}

#[test]
fn associative_array_assignment_accepts_unquoted_subscript_with_spaces() {
    let output = run_rubash_inline(
        "typeset -A _Dbg_next_complete\n_Dbg_next_complete[set autoeval]='_Dbg_complete_onoff'\nprintf '<%s>\\n' \"${_Dbg_next_complete[set autoeval]}\"\n",
    );

    assert!(output.status.success());
    assert_eq!(
        String::from_utf8_lossy(&output.stdout),
        "<_Dbg_complete_onoff>\n"
    );
    assert_eq!(String::from_utf8_lossy(&output.stderr), "");
}

#[test]
fn virtual_stdin_and_tty_match_noninteractive_bashdb_expectations() {
    let output = run_rubash_inline(
        "if [[ -r /dev/stdin ]]; then readable=yes; else readable=no; fi\ntty_output=$(tty); tty_status=$?\nprintf 'readable=%s tty=<%s> status=%s\\n' \"$readable\" \"$tty_output\" \"$tty_status\"\n",
    );

    assert!(output.status.success());
    assert_eq!(
        String::from_utf8_lossy(&output.stdout),
        "readable=yes tty=<not a tty> status=1\n"
    );
    assert_eq!(String::from_utf8_lossy(&output.stderr), "");
}

#[test]
fn dynamic_fd_redirect_can_read_from_dev_stdin() {
    let mut child = Command::new(env!("CARGO_BIN_EXE_rubash"))
        .arg("-c")
        .arg("exec {fd}</dev/stdin\nIFS= read -r -u $fd line\nprintf 'line=%s status=%s\\n' \"$line\" \"$?\"\n")
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .expect("run rubash");

    child.stdin.as_mut().unwrap().write_all(b"abc\n").unwrap();
    let output = child.wait_with_output().unwrap();

    assert!(output.status.success());
    assert_eq!(
        String::from_utf8_lossy(&output.stdout),
        "line=abc status=0\n"
    );
    assert_eq!(String::from_utf8_lossy(&output.stderr), "");
}

#[test]
fn anchored_parameter_replacement_handles_positional_parameter() {
    let output = run_rubash_inline(
        "HOME=/home/me\nset -- '~/.x'\nx=abc\nprintf '<%s>|<%s>|<%s>\\n' \"${x/#a/X}\" \"${x/%c/X}\" \"${1/#\\~/$HOME}\"\n",
    );

    assert!(output.status.success());
    assert_eq!(
        String::from_utf8_lossy(&output.stdout),
        "<Xbc>|<abX>|</home/me/.x>\n"
    );
    assert_eq!(String::from_utf8_lossy(&output.stderr), "");
}

#[test]
fn array_subscript_arithmetic_side_effect_updates_variable() {
    let output = run_rubash_inline("i=0\na[++i]=11\nprintf 'i=%s a1=%s\\n' \"$i\" \"${a[1]}\"\n");

    assert!(output.status.success());
    assert_eq!(String::from_utf8_lossy(&output.stdout), "i=1 a1=11\n");
    assert_eq!(String::from_utf8_lossy(&output.stderr), "");
}

#[test]
fn history_read_write_accept_file_operands() {
    let history_in = Path::new("target").join("rubash-cli-history-in.txt");
    let history_out = Path::new("target").join("rubash-cli-history-out.txt");
    fs::create_dir_all("target").unwrap();
    fs::write(&history_in, "echo saved\n").unwrap();
    let script = format!(
        "history -r {}\nprintf 'r=%s\\n' \"$?\"\nhistory -w {}\nprintf 'w=%s\\n' \"$?\"\n",
        history_in.to_string_lossy().replace('\\', "/"),
        history_out.to_string_lossy().replace('\\', "/"),
    );
    let output = run_rubash_inline(&script);

    let _ = fs::remove_file(&history_in);
    let _ = fs::remove_file(&history_out);
    assert!(output.status.success());
    assert_eq!(String::from_utf8_lossy(&output.stdout), "r=0\nw=0\n");
    assert_eq!(String::from_utf8_lossy(&output.stderr), "");
}

#[test]
fn windows_drive_absolute_tail_is_treated_as_absolute_path() {
    let file_path = Path::new("target").join("rubash-cli-drive-tail.txt");
    fs::create_dir_all("target").unwrap();
    fs::write(&file_path, "ok\n").unwrap();
    let cwd = std::env::current_dir()
        .unwrap()
        .to_string_lossy()
        .replace('\\', "/");
    let absolute = format!("{}/target/rubash-cli-drive-tail.txt", cwd);
    let bashdb_joined = format!("{}/target/{}", cwd, absolute);
    let script = format!("[[ -f {} ]] && echo yes || echo no\n", bashdb_joined);
    let output = run_rubash_inline(&script);

    let _ = fs::remove_file(&file_path);
    assert!(output.status.success());
    assert_eq!(String::from_utf8_lossy(&output.stdout), "yes\n");
    assert_eq!(String::from_utf8_lossy(&output.stderr), "");
}

#[test]
fn exit_trap_expands_bash_command_as_last_user_command() {
    let output = run_rubash_inline(
        r#"trap 'printf "exit:%s\n" "$BASH_COMMAND"' EXIT
echo hi
"#,
    );

    assert!(output.status.success());
    assert_eq!(
        String::from_utf8_lossy(&output.stdout),
        "hi\nexit:echo hi\n"
    );
    assert_eq!(String::from_utf8_lossy(&output.stderr), "");
}

#[test]
fn function_debug_trap_runs_inside_functrace_function_body() {
    let output = run_rubash_inline(
        "set -T\ntrap 'printf \\\"DBG:%s:%s\\\\n\\\" \"$LINENO\" \"$BASH_COMMAND\"' DEBUG\nf(){ echo in; }\nf\n",
    );

    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(output.status.success());
    assert!(stdout.contains("DBG:3:echo in\n"));
    assert!(stdout.ends_with("in\n"));
    assert_eq!(String::from_utf8_lossy(&output.stderr), "");
}

#[test]
fn compound_array_assignment_preserves_escaped_dollar_literals() {
    let output = run_rubash_inline(
        r#"arr=('\$cdir' '\$cwd')
printf '<%s>|<%s>\n' "${arr[0]}" "${arr[1]}"
"#,
    );

    assert!(output.status.success());
    assert_eq!(
        String::from_utf8_lossy(&output.stdout),
        "<\\$cdir>|<\\$cwd>\n"
    );
    assert_eq!(String::from_utf8_lossy(&output.stderr), "");
}

#[test]
fn line_continuation_after_apostrophe_inside_double_quotes_keeps_one_command() {
    let output = run_rubash_inline(
        r#"f(){ for x in "$@"; do printf '<%s>\n' "$x"; done; }
label='set prompt      -- '
_Dbg_debugger_name=bashdb
_Dbg_prompt_str='$_Dbg_debugger_name${_Dbg_less}${#_Dbg_history[@]}${_Dbg_greater}$_Dbg_space'
f \
  "${label}${_Dbg_debugger_name}'s prompt is:\n" \
  "      \"$_Dbg_prompt_str\"."
"#,
    );

    assert!(output.status.success());
    assert_eq!(
        String::from_utf8_lossy(&output.stdout),
        "<set prompt      -- bashdb's prompt is:\\n>\n<      \"$_Dbg_debugger_name${_Dbg_less}${#_Dbg_history[@]}${_Dbg_greater}$_Dbg_space\".>\n"
    );
    assert_eq!(String::from_utf8_lossy(&output.stderr), "");
}

#[test]
fn scalar_length_of_empty_and_populated_arrays_uses_element_zero() {
    let output = run_rubash_inline(
        "typeset -a empty=()\nfilled=(abcd ef)\ntypeset -A assoc=([0]=zero [k]=value)\nprintf '%s:%s:%s:%s:%s\\n' \"${#empty}\" \"${#empty[@]}\" \"${#filled}\" \"${#filled[@]}\" \"${#assoc}\"\n",
    );

    assert!(output.status.success());
    assert_eq!(String::from_utf8_lossy(&output.stdout), "0:0:4:2:4\n");
    assert_eq!(String::from_utf8_lossy(&output.stderr), "");
}

#[test]
fn bashdb_help_command_list_is_populated() {
    let mut child = Command::new(env!("CARGO_BIN_EXE_rubash"))
        .arg("target/bashdb-clean/bashdb-generated")
        .arg("--no-highlight")
        .arg("target/bashdb-probe-target.sh")
        .env("TERM", "xterm")
        .env("DARK_BG", "0")
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .expect("run rubash bashdb");

    child
        .stdin
        .as_mut()
        .unwrap()
        .write_all(b"help\nquit\n")
        .unwrap();
    let output = child.wait_with_output().unwrap();
    let stdout = String::from_utf8_lossy(&output.stdout);

    assert!(output.status.success());
    assert!(stdout.contains("Available commands:"));
    assert!(stdout.contains("  action"));
    assert!(stdout.contains("continue"));
    assert!(stdout.contains("undisplay"));
    assert!(!stdout.contains("<empty>"));
    assert_eq!(String::from_utf8_lossy(&output.stderr), "");
}

#[test]
fn quoted_special_status_parameter_expands_inside_double_quotes() {
    let output = run_rubash_inline("printf '<%s>\\n' \"$?\"\n");

    assert!(output.status.success());
    assert_eq!(String::from_utf8_lossy(&output.stdout), "<0>\n");
    assert_eq!(String::from_utf8_lossy(&output.stderr), "");
}

#[test]
fn quoted_assignment_like_argument_is_not_array_assignment_parse_error() {
    let output = run_rubash_inline(
        r#"key='D:/path'
printf '<%s>\n' "m[\"$key\"]+=\" 4 \""
"#,
    );

    assert!(output.status.success());
    assert_eq!(
        String::from_utf8_lossy(&output.stdout),
        "<m[\"D:/path\"]+=\" 4 \">\n"
    );
    assert_eq!(String::from_utf8_lossy(&output.stderr), "");
}

#[test]
fn eval_can_reparse_quoted_associative_array_append() {
    let output = run_rubash_inline(
        r#"typeset -A m=()
key='D:/path'
eval "m[\"$key\"]+=\" 4 \""
printf '<%s>\n' "${m[D:/path]}"
"#,
    );

    assert!(output.status.success());
    assert_eq!(String::from_utf8_lossy(&output.stdout), "< 4 >\n");
    assert_eq!(String::from_utf8_lossy(&output.stderr), "");
}

#[test]
fn compound_assignment_splits_unquoted_associative_array_element() {
    let output = run_rubash_inline(
        r#"typeset -A map=()
source_file='D:/repo/rubash/target/bashdb-probe-target.sh'
map["$source_file"]=' 1 '
brkpt_nos=(${map["$source_file"]})
printf '<%s:%s>\n' "${#brkpt_nos[@]}" "${brkpt_nos[0]}"
"#,
    );

    assert!(output.status.success());
    assert_eq!(String::from_utf8_lossy(&output.stdout), "<1:1>\n");
    assert_eq!(String::from_utf8_lossy(&output.stderr), "");
}

#[test]
fn declare_assignment_can_shadow_readonly_caller_local() {
    let output = run_rubash_inline(
        r#"outer(){ typeset -r del=1; inner 2; echo outer:$del; }
inner(){ typeset -i del=$1; echo inner:$del; }
outer
"#,
    );

    assert!(output.status.success());
    assert_eq!(
        String::from_utf8_lossy(&output.stdout),
        "inner:2\nouter:1\n"
    );
    assert_eq!(String::from_utf8_lossy(&output.stderr), "");
}

#[test]
fn bashdb_delete_removes_breakpoint_entry() {
    let mut child = Command::new(env!("CARGO_BIN_EXE_rubash"))
        .arg("target/bashdb-clean/bashdb-generated")
        .arg("--no-highlight")
        .arg("target/bashdb-probe-target.sh")
        .env("TERM", "xterm")
        .env("DARK_BG", "0")
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .expect("run rubash bashdb");

    child
        .stdin
        .as_mut()
        .unwrap()
        .write_all(b"break 4\ninfo breakpoints\ndelete 1\ninfo breakpoints\nquit\n")
        .unwrap();
    let output = child.wait_with_output().unwrap();
    let stdout = String::from_utf8_lossy(&output.stdout);

    assert!(output.status.success());
    assert!(stdout.contains("Breakpoint 1 set"));
    assert!(stdout.contains("Deleted breakpoint 1"));
    assert!(stdout.contains("No breakpoints have been set."));
    assert_eq!(String::from_utf8_lossy(&output.stderr), "");
}

#[test]
fn eval_compound_assignment_does_not_leak_field_split_marker() {
    let output = run_rubash_inline(
        r#"typeset -A map=()
key='D:/repo/rubash/target/bashdb-probe-target.sh'
map["$key"]=' 4 '
eval "via_eval=(${map[\"$key\"]})"
printf '<%s:%s:%s>\n' "${#via_eval[@]}" "${via_eval[0]}" "$(( via_eval[0] == 4 ))"
"#,
    );

    assert!(output.status.success());
    assert_eq!(String::from_utf8_lossy(&output.stdout), "<1:4:1>\n");
    assert_eq!(String::from_utf8_lossy(&output.stderr), "");
}

#[test]
fn bashdb_clear_removes_breakpoint_by_file_line() {
    let mut child = Command::new(env!("CARGO_BIN_EXE_rubash"))
        .arg("target/bashdb-clean/bashdb-generated")
        .arg("--no-highlight")
        .arg("target/bashdb-probe-target.sh")
        .env("TERM", "xterm")
        .env("DARK_BG", "0")
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .expect("run rubash bashdb");

    child
        .stdin
        .as_mut()
        .unwrap()
        .write_all(b"break 4\nclear 4\ninfo breakpoints\nquit\n")
        .unwrap();
    let output = child.wait_with_output().unwrap();
    let stdout = String::from_utf8_lossy(&output.stdout);

    assert!(output.status.success());
    assert!(stdout.contains("Breakpoint 1 set"));
    assert!(stdout.contains("Removed 1 breakpoint(s)."));
    assert!(stdout.contains("No breakpoints have been set."));
    assert_eq!(String::from_utf8_lossy(&output.stderr), "");
}

#[test]
fn unset_array_subscript_preserves_arithmetic_side_effects() {
    let output = run_rubash_inline(
        r#"a=([0]=10 [1]=20)
i=1
unset a[i--]
printf '<%s:%s:%s>\n' "$i" "${a[0]}" "${a[1]-}"
"#,
    );

    assert!(output.status.success());
    assert_eq!(String::from_utf8_lossy(&output.stdout), "<0:10:>\n");
    assert_eq!(String::from_utf8_lossy(&output.stderr), "");
}

#[test]
fn bashdb_commands_block_consumes_secondary_input_without_fd_loop() {
    let mut child = Command::new(env!("CARGO_BIN_EXE_rubash"))
        .arg("target/bashdb-clean/bashdb-generated")
        .arg("--no-highlight")
        .arg("target/bashdb-probe-target.sh")
        .env("TERM", "xterm")
        .env("DARK_BG", "0")
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .expect("run rubash bashdb");

    child
        .stdin
        .as_mut()
        .unwrap()
        .write_all(b"break 4\ncommands 1\nsilent\nend\nquit\n")
        .unwrap();
    let output = child.wait_with_output().unwrap();
    let stdout = String::from_utf8_lossy(&output.stdout);

    assert!(output.status.success());
    assert!(stdout.contains("Type commands for when breakpoint 1 hit"));
    assert_eq!(String::from_utf8_lossy(&output.stderr), "");
}

#[test]
fn assignment_builtin_strips_syntactic_quotes_from_quoted_command_substitution() {
    let output = run_rubash_inline(
        r#"u(){ builtin echo -n -e "$@"; }
typeset -r e="$(u x)"
typeset -r q="$(builtin printf '\"x\"')"
typeset literal='"x"'
declare -p e q literal
"#,
    );
    let stdout = String::from_utf8_lossy(&output.stdout);

    assert!(output.status.success());
    assert!(stdout.contains("declare -r e=\"x\""));
    assert!(stdout.contains("declare -r q=\"\\\"x\\\"\""));
    assert!(stdout.contains("declare -- literal=\"\\\"x\\\"\""));
    assert_eq!(String::from_utf8_lossy(&output.stderr), "");
}

#[test]
fn bashdb_examine_prints_debugged_local_variable() {
    let mut child = Command::new(env!("CARGO_BIN_EXE_rubash"))
        .arg("target/bashdb-clean/bashdb-generated")
        .arg("--no-highlight")
        .arg("target/bashdb-probe-target.sh")
        .env("TERM", "xterm")
        .env("DARK_BG", "0")
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .expect("run rubash bashdb");

    child
        .stdin
        .as_mut()
        .unwrap()
        .write_all(b"step\nnext\nexamine x\nquit\n")
        .unwrap();
    let output = child.wait_with_output().unwrap();
    let stdout = String::from_utf8_lossy(&output.stdout);

    assert!(output.status.success());
    assert!(stdout.contains("declare -- x=\"41\""));
    assert_eq!(String::from_utf8_lossy(&output.stderr), "");
}

#[test]
fn array_element_assignment_preserves_quoted_parenthesized_scalar_values() {
    let output = run_rubash_inline(
        r#"declare -A map=()
line='declare -A BASH_ALIASES=([0]="()" )'
if [[ $line =~ ^([^=]+)=(.*)$ ]]; then
  map["${BASH_REMATCH[1]}"]="${BASH_REMATCH[2]}"
fi
v='(x)'
map[k]=$v
arr=()
arr[0]="(z)"
printf '<%s:%s:%s>\n' "${map["declare -A BASH_ALIASES"]}" "${map[k]}" "${arr[0]}"
"#,
    );

    assert!(output.status.success());
    assert_eq!(
        String::from_utf8_lossy(&output.stdout),
        "<([0]=\"()\" ):(x):(z)>\n"
    );
    assert_eq!(String::from_utf8_lossy(&output.stderr), "");
}

#[test]
fn bashdb_info_variables_runs_without_assoc_value_errors() {
    let mut child = Command::new(env!("CARGO_BIN_EXE_rubash"))
        .arg("target/bashdb-clean/bashdb-generated")
        .arg("--no-highlight")
        .arg("target/bashdb-probe-target.sh")
        .env("TERM", "xterm")
        .env("DARK_BG", "0")
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .expect("run rubash bashdb");

    child
        .stdin
        .as_mut()
        .unwrap()
        .write_all(b"info variables\nquit\n")
        .unwrap();
    let output = child.wait_with_output().unwrap();
    let stdout = String::from_utf8_lossy(&output.stdout);

    assert!(output.status.success());
    assert!(stdout.contains("declare -A BASH_ALIASES"));
    assert_eq!(String::from_utf8_lossy(&output.stderr), "");
}
