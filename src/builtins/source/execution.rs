use crate::executor::{ExecuteError, Executor};
use crate::parser::{Ast, CommandNode};

pub fn execute_text(executor: &mut Executor, source: &str) -> Result<(), ExecuteError> {
    execute_text_with_args(executor, source, &[])
}

pub fn execute_text_with_args(
    executor: &mut Executor,
    source: &str,
    args: &[String],
) -> Result<(), ExecuteError> {
    let ast = parse_source_ast(source);
    execute_ast_with_args(executor, ast, args, None)
}

pub(super) fn execute_text_maybe_redirected(
    executor: &mut Executor,
    source: &str,
    args: &[String],
    redirect_cmd: Option<&CommandNode>,
    source_name: Option<&str>,
) -> Result<(), ExecuteError> {
    let mut ast = parse_source_ast(source);
    if let Some(redirect_cmd) = redirect_cmd {
        executor.apply_command_output_redirects(redirect_cmd, &mut ast)?;
    }
    execute_ast_with_args(executor, ast, args, source_name)
}

fn parse_source_ast(source: &str) -> Ast {
    let tokens = crate::lexer::tokenize(source);
    crate::parser::parse(&tokens)
}

fn execute_ast_with_args(
    executor: &mut Executor,
    ast: Ast,
    args: &[String],
    source_name: Option<&str>,
) -> Result<(), ExecuteError> {
    let old_positional_params = executor.positional_params();
    let source_positional_params: Vec<String> = args.to_vec();
    let had_source_args = !source_positional_params.is_empty();
    let old_source_marker = executor.get_env("__RUBASH_IN_SOURCE").map(str::to_string);
    let old_script_name = executor.get_env("__RUBASH_SCRIPT_NAME").map(str::to_string);
    let old_bash_argv0 = executor.get_env("BASH_ARGV0").map(str::to_string);
    executor.set_env("__RUBASH_IN_SOURCE", "1");
    if executor.get_env("__RUBASH_TOP_LEVEL_NAME").is_none() {
        let top_level_name = old_bash_argv0
            .as_deref()
            .or(old_script_name.as_deref())
            .unwrap_or("rubash");
        executor.set_env("__RUBASH_TOP_LEVEL_NAME", top_level_name);
    }
    if let Some(source_name) = source_name {
        executor.push_bash_source(source_name.to_string());
        if let Some(top_level_name) = old_script_name.as_deref().or(old_bash_argv0.as_deref()) {
            executor.set_env("BASH_ARGV0", top_level_name);
        }
        executor.set_env("__RUBASH_SCRIPT_NAME", source_name);
    }
    if had_source_args {
        executor.set_positional_params(source_positional_params.clone());
    }

    let result = executor.execute_ast(&ast);
    // GNU Bash runs the RETURN trap when a sourced script finishes
    // (builtins/evalfile.c run_return_trap after source_file).
    executor.run_return_trap()?;

    if source_name.is_some() {
        executor.pop_bash_source();
    }

    match old_source_marker {
        Some(value) => executor.set_env("__RUBASH_IN_SOURCE", &value),
        None => executor.remove_env("__RUBASH_IN_SOURCE"),
    }
    match old_script_name {
        Some(value) => executor.set_env("__RUBASH_SCRIPT_NAME", &value),
        None => executor.remove_env("__RUBASH_SCRIPT_NAME"),
    }
    match old_bash_argv0 {
        Some(value) => executor.set_env("BASH_ARGV0", &value),
        None => executor.remove_env("BASH_ARGV0"),
    }

    if had_source_args {
        executor.set_positional_params(old_positional_params);
    }

    match result {
        Err(ExecuteError::Return(status)) => {
            executor.set_exit_code(status);
            Ok(())
        }
        other => other,
    }
}
