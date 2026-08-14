//! Executor Module - Bash Command Executor
//!
//! Executes parsed AST commands.

pub(crate) mod arithmetic;
pub(crate) mod glob;
pub(crate) mod path;
mod upstream_scripts;
use arithmetic::{
    arithmetic_division_by_zero_token, arithmetic_unbound_variable, eval_arith_value,
    eval_conditional_arith_value,
};

mod arrays;
use arrays::*;
mod alias_arithmetic_for;
mod alias_case;
mod alias_loop_match;
mod alias_loops;
mod alias_reparse;
mod alias_select;
mod alias_set_builtins;
mod arithmetic_aliases;
mod array_assignment_exec;
mod assignment_dispatch;
mod assignment_expansion;
mod builtin_direct_command;
mod builtin_redirects;
mod command_dispatch;
mod command_dispatch_late;
mod command_dispatch_primary;
mod command_execute;
mod command_input_scope;
mod command_no_alias;
mod command_no_alias_late;
mod command_prepare;
mod command_substitution;
mod command_substitution_pipelines;
mod command_substitution_values;
mod command_words;
mod compound_exec;
use compound_exec::*;
mod declare_local;
mod dynamic_arrays;
mod fd_table;
mod embedded_mutations;
mod embedded_parameters;
mod expand_braced_indices;
mod expand_braced_ops;
mod expand_braced_patterns;
mod expand_braced_replacement;
mod expand_braced_special;
mod expand_word;
mod export_builtin;
mod external_file_builtins;
mod external_finish;
mod external_inner;
mod external_redirects;
mod external_setup;
mod function_calls;
mod function_locals;
mod getopts_enable;
mod init;
mod job_builtins;
mod limit_builtins;
mod lookup_paths;
mod loop_select;
mod mapfile_builtin;
mod mapfile_helpers;
mod option_builtins;
mod parameter_core;
mod parameter_errors;
mod parameter_patterns;
mod parameter_transforms;
mod parameter_words;
mod printf_path_builtins;
mod prompt_expansion;
mod public_accessors;
mod pwd_loop_builtins;
mod read_builtin;
mod read_io;
mod read_redirected_fd;
mod readonly_functions;
mod shell_options;

pub(crate) use shell_options::GlobalStdout;

mod shift_echo_builtins;
mod source_type_state;
mod temporary_assignments;
mod trap_exec;
mod trap_stack_builtins;
mod type_builtin;
mod type_describe;
mod type_functions;
mod unset_arrays;
mod variable_state;

mod alias_helpers;
mod assignment_helpers;
mod ast_exec;
mod builtin_names;
mod command_subst_helpers;
mod command_text;
mod env_helpers;
mod execution_misc;
mod function_env;
mod local_helpers;
mod parameter_case;
mod parameter_decode;
mod parameter_ops;
mod parameter_replace;
mod parse_helpers;
mod pipeline_exec;
mod pipeline_stages;

mod read_helpers;
mod read_split;
mod redirect_inherit;
mod redirection;
mod select_exec;
mod support_names;

use alias_helpers::*;
use assignment_helpers::*;
use builtin_names::*;
use command_subst_helpers::*;
use command_text::*;
use env_helpers::*;
use execution_misc::*;
use external_setup::{
    command_needs_process_substitution_materialization, ProcessSubstitutionFiles,
};
use function_env::*;
use local_helpers::*;
use parameter_case::*;
use parameter_decode::*;
use parameter_ops::*;
use parameter_replace::*;
use parse_helpers::*;
use read_helpers::*;
use read_split::*;
use redirect_inherit::*;
use support_names::*;
use fd_table::{FdReadEndpoint, FdTable, FdWriteEndpoint, MaterializedRead};
use crate::jobs::JobTable;
use crate::shell::state::ShellState;

pub(crate) mod conditional;
use conditional::{case_pattern_matches, case_pattern_matches_nocase, simple_grep_pattern_matches};

use crate::builtins::alias::Alias;
use crate::expand::tilde::tilde as tilde_expand;
use crate::lexer::TokenKind;
use crate::parser::{
    AndOrListCommand, ArithmeticCommand, ArithmeticExpressionMetadata, ArithmeticForCommand, Ast,
    BackgroundCommand, CaseClause, CaseCommand, CaseTerminator, CommandBodyKind, CommandNode,
    ConditionalCommand, ForCommand, FunctionBodyKind, FunctionCommand, IfCommand, InvertedCommand,
    LoopCommand, PipelineCommand, Redirect, SelectCommand, SubshellCommand, TimeCommand,
    WordMetadata,
};
use std::cell::Cell;
use std::collections::{BTreeMap, HashMap, HashSet};
use std::env;
use std::fs::{self, File, OpenOptions};
use std::io::{self, Read, Seek, SeekFrom, Write};
use std::path::PathBuf;

pub struct HostExternalCommandOutput {
    pub stdout: Vec<u8>,
    pub stderr: Vec<u8>,
    pub status: i32,
}

struct HostExternalCommandHandler(
    Box<dyn FnMut(&[String], &HashMap<String, String>) -> Option<HostExternalCommandOutput>>,
);

impl std::fmt::Debug for HostExternalCommandHandler {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str("HostExternalCommandHandler(..)")
    }
}
use std::process::{Command, Stdio};
use std::rc::Rc;
use std::sync::Mutex;
use std::time::{SystemTime, UNIX_EPOCH};

use self::path::{
    apply_required_windows_child_environment, external_command_for_named_program, find_shell,
    find_user_command, shell_path_to_windows, standard_path,
};

const EXPORTED_VARS: &str = "__RUBASH_EXPORTED_VARS";
const EXPORTED_FUNCTIONS: &str = "__RUBASH_EXPORTED_FUNCTIONS";
const READONLY_VARS: &str = "__RUBASH_READONLY_VARS";
const READONLY_FUNCTIONS: &str = "__RUBASH_READONLY_FUNCTIONS";
const INTEGER_VARS: &str = "__RUBASH_INTEGER_VARS";
const UPPERCASE_VARS: &str = "__RUBASH_UPPERCASE_VARS";
const LOWERCASE_VARS: &str = "__RUBASH_LOWERCASE_VARS";
const NAMEREF_VARS: &str = "__RUBASH_NAMEREF_VARS";
const ARRAY_VARS: &str = "__RUBASH_ARRAY_VARS";
const ASSOC_VARS: &str = "__RUBASH_ASSOC_VARS";
const SHELL_START_EPOCH: &str = "__RUBASH_SHELL_START_EPOCH";
const SECONDS_OFFSET: &str = "__RUBASH_SECONDS_OFFSET";
const FUNCTION_STDIN: &str = "__RUBASH_FUNCTION_STDIN";
const FUNCTION_STDIN_OFFSET: &str = "__RUBASH_FUNCTION_STDIN_OFFSET";
const FD_STDIN_PREFIX: &str = "__RUBASH_FD_STDIN_";
const FD_STDIN_OFFSET_PREFIX: &str = "__RUBASH_FD_STDIN_OFFSET_";
const FD_DYNAMIC_INPUT_PREFIX: &str = "__RUBASH_FD_DYNAMIC_INPUT_";
const FD_OUTPUT_PREFIX: &str = "__RUBASH_FD_OUTPUT_";
const FD_OUTPUT_PROCESS_SUBSTITUTION_PREFIX: &str = "__RUBASH_FD_OUTPUT_PROCESS_SUBSTITUTION_";
const FD_CLOSED_PREFIX: &str = "__RUBASH_FD_CLOSED_";
const FD_STDOUT_TARGET: &str = "__RUBASH_FD_STDOUT";
const FD_STDERR_TARGET: &str = "__RUBASH_FD_STDERR";
const FD_COPROC_STDIN_TARGET_PREFIX: &str = "__RUBASH_COPROC_STDIN:";
const FD_PROCESS_STDIN_TARGET: &str = "__RUBASH_FD_PROCESS_STDIN";
const INHERIT_PROCESS_STDIN: &str = "__RUBASH_INHERIT_PROCESS_STDIN";
const LOCAL_EXPORT_ENV: &str = "__RUBASH_LOCAL_EXPORT_ENV";
const POSIX_FUNCTION_EXPORT_TOUCHED: &str = "__RUBASH_POSIX_FUNCTION_EXPORT_TOUCHED";
const DECLARED_UNSET_VARS: &str = "__RUBASH_DECLARED_UNSET_VARS";
const COMPOUND_ASSIGNMENT_MARKER: char = '\x1e';
const ARRAY_FIELD_SPLIT_MARKER: char = '\x1c';
const SKIP_POSIXPIPE_TIME_COUNT_REMAINDER: &str = "__RUBASH_SKIP_POSIXPIPE_TIME_COUNT_REMAINDER";

static EXECUTION_LOCK: Mutex<()> = Mutex::new(());

thread_local! {
    static EXECUTION_LOCK_DEPTH: Cell<usize> = const { Cell::new(0) };
}

enum NamerefResolution {
    Target(String),
    Circular,
    NotNameref,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum TypeDescribeMode {
    Verbose,
    Reusable,
    TypeOnly,
    PathOnly,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum LoopControlKind {
    Break,
    Continue,
}

type FunctionBody = Rc<Ast>;

impl LoopControlKind {
    fn name(self) -> &'static str {
        match self {
            LoopControlKind::Break => "break",
            LoopControlKind::Continue => "continue",
        }
    }
}

/// Execution error
#[derive(Debug)]
pub enum ExecuteError {
    CommandNotFound(String),
    /// A direct host-side function dispatch requested a function that is not
    /// defined in the executor.
    FunctionNotFound(String),
    IoError(std::io::Error),
    ExitCode(i32),
    Break(usize),
    Continue(usize),
    Return(i32),
    UnknownBuiltin(String),
}

impl std::fmt::Display for ExecuteError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            ExecuteError::CommandNotFound(cmd) => write!(f, "rubash: {}: command not found", cmd),
            ExecuteError::FunctionNotFound(name) => {
                write!(f, "rubash: {}: function not found", name)
            }
            ExecuteError::IoError(e) => write!(f, "rubash: {}", e),
            ExecuteError::ExitCode(code) => write!(f, "exit code: {}", code),
            ExecuteError::Break(level) => write!(f, "break {}", level),
            ExecuteError::Continue(level) => write!(f, "continue {}", level),
            ExecuteError::Return(status) => write!(f, "return {}", status),
            ExecuteError::UnknownBuiltin(name) => {
                write!(f, "rubash: {}: builtin command not found", name)
            }
        }
    }
}

impl std::error::Error for ExecuteError {}

impl From<std::io::Error> for ExecuteError {
    fn from(e: std::io::Error) -> Self {
        ExecuteError::IoError(e)
    }
}

#[derive(Clone, Copy, Debug, Default)]
struct VarAttrs {
    exported: bool,
    readonly: bool,
    integer: bool,
    uppercase: bool,
    lowercase: bool,
    nameref: bool,
    array: bool,
    assoc: bool,
}

#[derive(Debug)]
struct SavedGlobalDeclareLocal {
    name: String,
    scope_index: usize,
    local_value: Option<String>,
    local_attrs: VarAttrs,
}

/// Command executor
#[derive(Debug)]
pub struct Executor {
    shell_state: ShellState,
    fd_table: FdTable,
    job_table: JobTable,
    exit_code: i32,
    env_vars: HashMap<String, String>,
    aliases: HashMap<String, Alias>,
    functions: HashMap<String, FunctionBody>,
    function_definition_redirects: HashMap<String, CommandNode>,
    positional_params: Vec<String>,
    pipestatus: Vec<i32>,
    function_name_stack: Vec<String>,
    bash_argc_stack: Vec<String>,
    bash_argv_stack: Vec<String>,
    bash_lineno_stack: Vec<String>,
    bash_source_stack: Vec<String>,
    local_var_scopes: Vec<HashMap<String, Option<String>>>,
    local_attr_scopes: Vec<HashMap<String, VarAttrs>>,
    expanding_aliases: Vec<String>,
    loop_depth: usize,
    function_depth: usize,
    random_state: Cell<u32>,
    shell_pid: u32,
    subshell_depth: Cell<usize>,
    owns_signal_mailbox: bool,
    last_background_pid: Option<u32>,
    background_children: HashMap<u32, std::process::Child>,
    /// Exit statuses reaped by `wait -n`, which Bash still makes available to
    /// a later explicit `wait PID`.
    background_statuses: HashMap<u32, i32>,
    background_jobs: HashMap<u32, String>,
    background_job_order: Vec<u32>,
    coproc_stdin_writers: HashMap<u32, std::io::PipeWriter>,
    coproc_stdout_readers: HashMap<u32, std::io::PipeReader>,
    assignment_output_process_substitutions: HashMap<String, String>,
    suppress_errexit: usize,
    debug_trap_running: bool,
    return_trap_running: bool,
    signal_trap_running: bool,
    debug_trap_command: Option<String>,
    arithmetic_expansion_error: Cell<bool>,
    last_command_substitution_status: Cell<Option<i32>>,
    stdout_capture: Option<Vec<u8>>,
    stderr_capture: Option<Vec<u8>>,
    host_external_command_handler: Option<HostExternalCommandHandler>,
    external_file_builtins_enabled: bool,
    process_env_snapshot: HashMap<String, String>,
}

#[cfg(test)]
mod tests;
