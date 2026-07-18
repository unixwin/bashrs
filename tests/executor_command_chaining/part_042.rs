use super::super::*;
use std::fs;

#[test]
fn test_compgen_empty_state_redirects_no_output() {
    let output_path = "target/rubash-compgen-output.txt";
    let status_path = "target/rubash-compgen-status.txt";
    let _ = fs::remove_file(output_path);
    let _ = fs::remove_file(status_path);
    let input = format!("compgen > {output_path}; echo $? > {status_path}");
    let tokens = tokenize(&input);
    let ast = parse(&tokens);
    let mut executor = Executor::new();

    let result = executor.execute_ast(&ast);

    assert!(result.is_ok());
    assert_eq!(executor.last_exit_code(), 0);
    assert_eq!(fs::read_to_string(output_path).unwrap(), "");
    assert_eq!(fs::read_to_string(status_path).unwrap(), "0\n");
    let _ = fs::remove_file(output_path);
    let _ = fs::remove_file(status_path);
}

#[test]
fn test_compgen_invalid_option_reports_usage() {
    let error_path = "target/rubash-compgen-error.txt";
    let status_path = "target/rubash-compgen-error-status.txt";
    let _ = fs::remove_file(error_path);
    let _ = fs::remove_file(status_path);
    let input = format!("compgen -x 2> {error_path}; echo $? > {status_path}");
    let tokens = tokenize(&input);
    let ast = parse(&tokens);
    let mut executor = Executor::new();

    let result = executor.execute_ast(&ast);

    assert!(result.is_ok());
    assert_eq!(executor.last_exit_code(), 0);
    assert_eq!(fs::read_to_string(status_path).unwrap(), "2\n");
    let error = fs::read_to_string(error_path).unwrap();
    assert!(error.contains("compgen: -x: invalid option\n"));
    assert!(error.contains("compgen: usage: compgen "));
    let _ = fs::remove_file(error_path);
    let _ = fs::remove_file(status_path);
}

#[test]
fn test_compgen_wordlist_filters_prefix_matches() {
    let output_path = "target/rubash-compgen-wordlist-output.txt";
    let status_path = "target/rubash-compgen-wordlist-status.txt";
    let _ = fs::remove_file(output_path);
    let _ = fs::remove_file(status_path);
    let input = format!("compgen -W 'alpha beta gamma' b > {output_path}; echo $? > {status_path}");
    let tokens = tokenize(&input);
    let ast = parse(&tokens);
    let mut executor = Executor::new();

    let result = executor.execute_ast(&ast);

    assert!(result.is_ok());
    assert_eq!(executor.last_exit_code(), 0);
    assert_eq!(fs::read_to_string(output_path).unwrap(), "beta\n");
    assert_eq!(fs::read_to_string(status_path).unwrap(), "0\n");
    let _ = fs::remove_file(output_path);
    let _ = fs::remove_file(status_path);
}

#[test]
fn test_compgen_wordlist_prefix_suffix_and_no_match_status() {
    let output_path = "target/rubash-compgen-prefix-suffix-output.txt";
    let status_path = "target/rubash-compgen-prefix-suffix-status.txt";
    let _ = fs::remove_file(output_path);
    let _ = fs::remove_file(status_path);
    let input = format!(
        "compgen -P pre- -S -suf -W 'alpha beta' a > {output_path}; echo first:$? > {status_path}; \
         compgen -W 'alpha beta' z >> {output_path}; echo second:$? >> {status_path}"
    );
    let tokens = tokenize(&input);
    let ast = parse(&tokens);
    let mut executor = Executor::new();

    let result = executor.execute_ast(&ast);

    assert!(result.is_ok());
    assert_eq!(executor.last_exit_code(), 0);
    assert_eq!(fs::read_to_string(output_path).unwrap(), "pre-alpha-suf\n");
    assert_eq!(
        fs::read_to_string(status_path).unwrap(),
        "first:0\nsecond:1\n"
    );
    let _ = fs::remove_file(output_path);
    let _ = fs::remove_file(status_path);
}

#[test]
fn test_compgen_filter_pattern_excludes_matching_candidates() {
    let output_path = "target/rubash-compgen-filter-output.txt";
    let status_path = "target/rubash-compgen-filter-status.txt";
    let _ = fs::remove_file(output_path);
    let _ = fs::remove_file(status_path);
    let input =
        format!("compgen -W 'alpha beta bar' -X 'b*' > {output_path}; echo $? > {status_path}");
    let tokens = tokenize(&input);
    let ast = parse(&tokens);
    let mut executor = Executor::new();

    let result = executor.execute_ast(&ast);

    assert!(result.is_ok());
    assert_eq!(executor.last_exit_code(), 0);
    assert_eq!(fs::read_to_string(output_path).unwrap(), "alpha\n");
    assert_eq!(fs::read_to_string(status_path).unwrap(), "0\n");
    let _ = fs::remove_file(output_path);
    let _ = fs::remove_file(status_path);
}

#[test]
fn test_compgen_filter_pattern_inversion_keeps_matching_candidates() {
    let output_path = "target/rubash-compgen-filter-invert-output.txt";
    let status_path = "target/rubash-compgen-filter-invert-status.txt";
    let _ = fs::remove_file(output_path);
    let _ = fs::remove_file(status_path);
    let input = format!(
        "compgen -P pre- -S -suf -W 'alpha beta bar' -X '!b*' > {output_path}; \
         echo $? > {status_path}"
    );
    let tokens = tokenize(&input);
    let ast = parse(&tokens);
    let mut executor = Executor::new();

    let result = executor.execute_ast(&ast);

    assert!(result.is_ok());
    assert_eq!(executor.last_exit_code(), 0);
    assert_eq!(
        fs::read_to_string(output_path).unwrap(),
        "pre-beta-suf\npre-bar-suf\n"
    );
    assert_eq!(fs::read_to_string(status_path).unwrap(), "0\n");
    let _ = fs::remove_file(output_path);
    let _ = fs::remove_file(status_path);
}

#[test]
fn test_compgen_filter_pattern_all_filtered_keeps_generation_status() {
    let output_path = "target/rubash-compgen-filter-empty-output.txt";
    let status_path = "target/rubash-compgen-filter-empty-status.txt";
    let _ = fs::remove_file(output_path);
    let _ = fs::remove_file(status_path);
    let input =
        format!("compgen -W 'alpha beta' -X 'a*' a > {output_path}; echo $? > {status_path}");
    let tokens = tokenize(&input);
    let ast = parse(&tokens);
    let mut executor = Executor::new();

    let result = executor.execute_ast(&ast);

    assert!(result.is_ok());
    assert_eq!(executor.last_exit_code(), 0);
    assert_eq!(fs::read_to_string(output_path).unwrap(), "");
    assert_eq!(fs::read_to_string(status_path).unwrap(), "0\n");
    let _ = fs::remove_file(output_path);
    let _ = fs::remove_file(status_path);
}

#[test]
fn test_compgen_glob_pattern_outputs_path_matches() {
    let dir_path = "target/rubash-compgen-glob";
    let output_path = "target/rubash-compgen-glob-output.txt";
    let status_path = "target/rubash-compgen-glob-status.txt";
    let _ = fs::remove_dir_all(dir_path);
    let _ = fs::remove_file(output_path);
    let _ = fs::remove_file(status_path);
    fs::create_dir_all(dir_path).unwrap();
    fs::write(format!("{dir_path}/alpha.txt"), "").unwrap();
    fs::write(format!("{dir_path}/beta.txt"), "").unwrap();
    fs::write(format!("{dir_path}/gamma.log"), "").unwrap();
    let input = format!(
        "compgen -G '{dir_path}/*.txt' > {output_path}; echo first:$? > {status_path}; \
         compgen -G '{dir_path}/*.zzz' >> {output_path}; echo second:$? >> {status_path}"
    );
    let tokens = tokenize(&input);
    let ast = parse(&tokens);
    let mut executor = Executor::new();

    let result = executor.execute_ast(&ast);

    assert!(result.is_ok());
    assert_eq!(executor.last_exit_code(), 0);
    assert_eq!(
        fs::read_to_string(output_path).unwrap(),
        format!("{dir_path}/alpha.txt\n{dir_path}/beta.txt\n")
    );
    assert_eq!(
        fs::read_to_string(status_path).unwrap(),
        "first:0\nsecond:1\n"
    );
    let _ = fs::remove_dir_all(dir_path);
    let _ = fs::remove_file(output_path);
    let _ = fs::remove_file(status_path);
}

#[test]
fn test_compgen_glob_pattern_uses_prefix_and_suffix() {
    let dir_path = "target/rubash-compgen-glob-wrap";
    let output_path = "target/rubash-compgen-glob-wrap-output.txt";
    let status_path = "target/rubash-compgen-glob-wrap-status.txt";
    let _ = fs::remove_dir_all(dir_path);
    let _ = fs::remove_file(output_path);
    let _ = fs::remove_file(status_path);
    fs::create_dir_all(dir_path).unwrap();
    fs::write(format!("{dir_path}/one.rs"), "").unwrap();
    let input = format!(
        "compgen -P pre- -S -suf -G '{dir_path}/*.rs' > {output_path}; echo $? > {status_path}"
    );
    let tokens = tokenize(&input);
    let ast = parse(&tokens);
    let mut executor = Executor::new();

    let result = executor.execute_ast(&ast);

    assert!(result.is_ok());
    assert_eq!(executor.last_exit_code(), 0);
    assert_eq!(
        fs::read_to_string(output_path).unwrap(),
        format!("pre-{dir_path}/one.rs-suf\n")
    );
    assert_eq!(fs::read_to_string(status_path).unwrap(), "0\n");
    let _ = fs::remove_dir_all(dir_path);
    let _ = fs::remove_file(output_path);
    let _ = fs::remove_file(status_path);
}

#[test]
fn test_compgen_builtin_action_filters_prefix_matches() {
    let output_path = "target/rubash-compgen-builtin-output.txt";
    let status_path = "target/rubash-compgen-builtin-status.txt";
    let _ = fs::remove_file(output_path);
    let _ = fs::remove_file(status_path);
    let input = format!(
        "compgen -P pre- -S -suf -A builtin pr > {output_path}; echo first:$? > {status_path}; \
         compgen -A builtin zz >> {output_path}; echo second:$? >> {status_path}"
    );
    let tokens = tokenize(&input);
    let ast = parse(&tokens);
    let mut executor = Executor::new();

    let result = executor.execute_ast(&ast);

    assert!(result.is_ok());
    assert_eq!(executor.last_exit_code(), 0);
    assert_eq!(fs::read_to_string(output_path).unwrap(), "pre-printf-suf\n");
    assert_eq!(
        fs::read_to_string(status_path).unwrap(),
        "first:0\nsecond:1\n"
    );
    let _ = fs::remove_file(output_path);
    let _ = fs::remove_file(status_path);
}

#[test]
fn test_compgen_keyword_action_filters_prefix_matches() {
    let output_path = "target/rubash-compgen-keyword-output.txt";
    let status_path = "target/rubash-compgen-keyword-status.txt";
    let _ = fs::remove_file(output_path);
    let _ = fs::remove_file(status_path);
    let input = format!("compgen -P kw: -A keyword wh > {output_path}; echo $? > {status_path}");
    let tokens = tokenize(&input);
    let ast = parse(&tokens);
    let mut executor = Executor::new();

    let result = executor.execute_ast(&ast);

    assert!(result.is_ok());
    assert_eq!(executor.last_exit_code(), 0);
    assert_eq!(fs::read_to_string(output_path).unwrap(), "kw:while\n");
    assert_eq!(fs::read_to_string(status_path).unwrap(), "0\n");
    let _ = fs::remove_file(output_path);
    let _ = fs::remove_file(status_path);
}

#[test]
fn test_compgen_shopt_action_filters_prefix_matches() {
    let output_path = "target/rubash-compgen-shopt-output.txt";
    let status_path = "target/rubash-compgen-shopt-status.txt";
    let _ = fs::remove_file(output_path);
    let _ = fs::remove_file(status_path);
    let input = format!("compgen -A shopt null > {output_path}; echo $? > {status_path}");
    let tokens = tokenize(&input);
    let ast = parse(&tokens);
    let mut executor = Executor::new();

    let result = executor.execute_ast(&ast);

    assert!(result.is_ok());
    assert_eq!(executor.last_exit_code(), 0);
    assert_eq!(fs::read_to_string(output_path).unwrap(), "nullglob\n");
    assert_eq!(fs::read_to_string(status_path).unwrap(), "0\n");
    let _ = fs::remove_file(output_path);
    let _ = fs::remove_file(status_path);
}

#[test]
fn test_compgen_setopt_action_filters_prefix_matches() {
    let output_path = "target/rubash-compgen-setopt-output.txt";
    let status_path = "target/rubash-compgen-setopt-status.txt";
    let _ = fs::remove_file(output_path);
    let _ = fs::remove_file(status_path);
    let input = format!(
        "compgen -P opt: -A setopt pipe > {output_path}; echo first:$? > {status_path}; \
         compgen -A setopt no_such >> {output_path}; echo second:$? >> {status_path}"
    );
    let tokens = tokenize(&input);
    let ast = parse(&tokens);
    let mut executor = Executor::new();

    let result = executor.execute_ast(&ast);

    assert!(result.is_ok());
    assert_eq!(executor.last_exit_code(), 0);
    assert_eq!(fs::read_to_string(output_path).unwrap(), "opt:pipefail\n");
    assert_eq!(
        fs::read_to_string(status_path).unwrap(),
        "first:0\nsecond:1\n"
    );
    let _ = fs::remove_file(output_path);
    let _ = fs::remove_file(status_path);
}

#[test]
fn test_compopt_outside_completion_function_fails() {
    let error_path = "target/rubash-compopt-error.txt";
    let status_path = "target/rubash-compopt-status.txt";
    let _ = fs::remove_file(error_path);
    let _ = fs::remove_file(status_path);
    let input = format!("compopt 2> {error_path}; echo $? > {status_path}");
    let tokens = tokenize(&input);
    let ast = parse(&tokens);
    let mut executor = Executor::new();

    let result = executor.execute_ast(&ast);

    assert!(result.is_ok());
    assert_eq!(executor.last_exit_code(), 0);
    assert_eq!(fs::read_to_string(status_path).unwrap(), "1\n");
    assert!(fs::read_to_string(error_path)
        .unwrap()
        .contains("compopt: not currently executing completion function\n"));
    let _ = fs::remove_file(error_path);
    let _ = fs::remove_file(status_path);
}

#[test]
fn test_builtin_compopt_invalid_option_reports_usage() {
    let error_path = "target/rubash-builtin-compopt-error.txt";
    let status_path = "target/rubash-builtin-compopt-status.txt";
    let _ = fs::remove_file(error_path);
    let _ = fs::remove_file(status_path);
    let input = format!("builtin compopt -x 2> {error_path}; echo $? > {status_path}");
    let tokens = tokenize(&input);
    let ast = parse(&tokens);
    let mut executor = Executor::new();

    let result = executor.execute_ast(&ast);

    assert!(result.is_ok());
    assert_eq!(executor.last_exit_code(), 0);
    assert_eq!(fs::read_to_string(status_path).unwrap(), "2\n");
    let error = fs::read_to_string(error_path).unwrap();
    assert!(error.contains("compopt: -x: invalid option\n"));
    assert!(error.contains("compopt: usage: compopt "));
    let _ = fs::remove_file(error_path);
    let _ = fs::remove_file(status_path);
}

#[test]
fn test_eval_redirects_loop_body_without_retruncating() {
    let output_path = "target/rubash-eval-loop-redirect-output.txt";
    let _ = fs::remove_file(output_path);
    let input = format!("eval 'for x in a b; do echo $x; done' > {output_path}");
    let tokens = tokenize(&input);
    let ast = parse(&tokens);
    let mut executor = Executor::new();

    let result = executor.execute_ast(&ast);

    assert!(result.is_ok());
    assert_eq!(executor.last_exit_code(), 0);
    assert_eq!(fs::read_to_string(output_path).unwrap(), "a\nb\n");
    let _ = fs::remove_file(output_path);
}

#[test]
fn test_type_redirects_output() {
    let output_path = "target/rubash-type-redirect-output.txt";
    let _ = fs::remove_file(output_path);
    let input = format!("type -t echo > {output_path}");
    let tokens = tokenize(&input);
    let ast = parse(&tokens);
    let mut executor = Executor::new();

    let result = executor.execute_ast(&ast);

    assert!(result.is_ok());
    assert_eq!(executor.last_exit_code(), 0);
    assert_eq!(fs::read_to_string(output_path).unwrap(), "builtin\n");
    let _ = fs::remove_file(output_path);
}

#[test]
fn test_type_appends_output() {
    let output_path = "target/rubash-type-append-output.txt";
    let _ = fs::remove_file(output_path);
    fs::write(output_path, "before\n").unwrap();
    let input = format!("type -t echo >> {output_path}");
    let tokens = tokenize(&input);
    let ast = parse(&tokens);
    let mut executor = Executor::new();

    let result = executor.execute_ast(&ast);

    assert!(result.is_ok());
    assert_eq!(executor.last_exit_code(), 0);
    assert_eq!(
        fs::read_to_string(output_path).unwrap(),
        "before\nbuiltin\n"
    );
    let _ = fs::remove_file(output_path);
}

#[test]
fn test_type_prints_function_heredoc_body() {
    let output_path = "target/rubash-type-function-heredoc-output.txt";
    let _ = fs::remove_file(output_path);
    let input =
        format!("f()\n{{\ncat <<EOF > /dev/null\nbody\nEOF\naa=1\n}}\ntype f > {output_path}");
    let tokens = tokenize(&input);
    let ast = parse(&tokens);
    let mut executor = Executor::new();

    let result = executor.execute_ast(&ast);

    assert!(result.is_ok());
    assert_eq!(executor.last_exit_code(), 0);
    assert_eq!(
        fs::read_to_string(output_path).unwrap(),
        "f is a function\nf () \n{ \n    cat <<EOF > /dev/null\nbody\nEOF\n\n    aa=1\n}\n"
    );
    let _ = fs::remove_file(output_path);
}

#[test]
fn test_type_terminates_plain_commands_before_function_heredoc() {
    let output_path = "target/rubash-type-function-heredoc-terminator-output.txt";
    let _ = fs::remove_file(output_path);
    let input = format!("f()\n{{\necho\ncat <<EOF\nbody\nEOF\n}}\ntype f > {output_path}");
    let tokens = tokenize(&input);
    let ast = parse(&tokens);
    let mut executor = Executor::new();

    let result = executor.execute_ast(&ast);

    assert!(result.is_ok());
    assert_eq!(executor.last_exit_code(), 0);
    assert_eq!(
        fs::read_to_string(output_path).unwrap(),
        "f is a function\nf () \n{ \n    echo;\n    cat <<EOF\nbody\nEOF\n\n}\n"
    );
    let _ = fs::remove_file(output_path);
}

#[test]
fn test_type_prints_compound_function_bodies() {
    let output_path = "target/rubash-type-compound-function-bodies-output.txt";
    let _ = fs::remove_file(output_path);
    let input = format!(
        "f() for x in a; {{ echo $x; }}; \
         s() select y in b; {{ echo $y; break; }}; \
         c() case $1 in a) echo alpha ;; *) echo other ;; esac; \
         type f s c > {output_path}"
    );
    let tokens = tokenize(&input);
    let ast = parse(&tokens);
    let mut executor = Executor::new();

    let result = executor.execute_ast(&ast);

    assert!(result.is_ok());
    assert_eq!(executor.last_exit_code(), 0);
    let output = fs::read_to_string(output_path).unwrap();
    assert!(output.contains("    for x in a; { echo $x; }"));
    assert!(output.contains("    select y in b; { echo $y; break; }"));
    assert!(output.contains("    case $1 in a) echo alpha ;; *) echo other ;; esac"));
    let _ = fs::remove_file(output_path);
}

#[test]
fn test_type_prints_nested_function_definition_body() {
    let output_path = "target/rubash-type-nested-function-body-output.txt";
    let _ = fs::remove_file(output_path);
    let input =
        format!("outer() {{ inner() {{ echo nested; }}; inner; }}; type outer > {output_path}");
    let tokens = tokenize(&input);
    let ast = parse(&tokens);
    let mut executor = Executor::new();

    let result = executor.execute_ast(&ast);

    assert!(result.is_ok());
    assert_eq!(executor.last_exit_code(), 0);
    let output = fs::read_to_string(output_path).unwrap();
    assert!(output.contains("    inner() { echo nested; }"));
    assert!(output.contains("    inner\n"));
    let _ = fs::remove_file(output_path);
}

#[test]
fn test_declare_pf_prints_condition_heredocs() {
    let output_path = "target/rubash-declare-function-condition-heredoc-output.txt";
    let _ = fs::remove_file(output_path);
    let input = format!(
        "foo()\n{{\necho begin\nif cat << HERE\ncontents\nHERE\nthen\n    echo 1 2\n    echo 3 4\nfi\n}}\n\
         declare -pf foo > {output_path}\n\
         foo()\n{{\necho begin\nwhile read var << HERE\ncontents\nHERE\ndo\n    echo 1 2\n    echo 3 4\ndone\n}}\n\
         declare -pf foo >> {output_path}"
    );
    let tokens = tokenize(&input);
    let ast = parse(&tokens);
    let mut executor = Executor::new();

    let result = executor.execute_ast(&ast);

    assert!(result.is_ok());
    assert_eq!(executor.last_exit_code(), 0);
    let output = fs::read_to_string(output_path).unwrap();
    assert!(output.contains("    if cat <<HERE\ncontents\nHERE\n    then\n"));
    assert!(output.contains("        echo 1 2;\n        echo 3 4;\n    fi\n"));
    assert!(output.contains("    while read var <<HERE\ncontents\nHERE\n    do\n"));
    assert!(output.contains("        echo 1 2;\n        echo 3 4;\n    done\n"));
    let _ = fs::remove_file(output_path);
}

#[test]
fn test_alias_heredoc_reads_following_lines_and_nested_alias_body() {
    let first_output_path = "target/rubash-alias-heredoc-following-output.txt";
    let nested_output_path = "target/rubash-alias-heredoc-nested-output.txt";
    let _ = fs::remove_file(first_output_path);
    let _ = fs::remove_file(nested_output_path);
    let input = format!(
        "shopt -s expand_aliases\n\
         alias 'headplus=cat > {first_output_path} <<EOF\nhello'\n\
         headplus\nworld\nEOF\n\
         alias head='cat > {nested_output_path} <<\\END' body='head\nhere-document\nEND'\n\
         body"
    );
    let tokens = tokenize(&input);
    let ast = parse(&tokens);
    let mut executor = Executor::new();

    let result = executor.execute_ast(&ast);

    assert!(result.is_ok());
    assert_eq!(executor.last_exit_code(), 0);
    assert_eq!(
        fs::read_to_string(first_output_path).unwrap(),
        "hello\nworld\n"
    );
    assert_eq!(
        fs::read_to_string(nested_output_path).unwrap(),
        "here-document\n"
    );
    let _ = fs::remove_file(first_output_path);
    let _ = fs::remove_file(nested_output_path);
}
