//! declare module.
//!
//! GNU Bash source ownership:
// - builtins/declare.def

use std::collections::HashMap;
use std::io::{self, Write};

mod assign;
mod attrs;
mod diagnostic;
mod marks;
mod names;
mod output;
mod print;
mod storage;

use crate::shell::VariableStore;
use assign::assign_declare_names;
use attrs::{apply_declare_attrs, DeclareOptions};
use diagnostic::diagnostic_prefix;
use marks::marked_vars;
use names::{declare_base_name, nameref_self_reference, valid_declare_name};
use storage::{indexed_array_entries, parse_assoc_words};

const EXECUTION_SUCCESS: i32 = 0;
const EXECUTION_FAILURE: i32 = 1;
const EXPORTED_VARS: &str = "__RUBASH_EXPORTED_VARS";
const READONLY_VARS: &str = "__RUBASH_READONLY_VARS";
const ARRAY_VARS: &str = "__RUBASH_ARRAY_VARS";
const ASSOC_VARS: &str = "__RUBASH_ASSOC_VARS";
const INTEGER_VARS: &str = "__RUBASH_INTEGER_VARS";
const UPPERCASE_VARS: &str = "__RUBASH_UPPERCASE_VARS";
const LOWERCASE_VARS: &str = "__RUBASH_LOWERCASE_VARS";
const NAMEREF_VARS: &str = "__RUBASH_NAMEREF_VARS";
const DECLARED_UNSET_VARS: &str = "__RUBASH_DECLARED_UNSET_VARS";
const COMPOUND_ASSIGNMENT_MARKER: char = '\x1e';
const EX_USAGE: i32 = 2;

/// Synchronize indexed declarations into the typed variable owner after the
/// legacy builtin path has applied its attribute and encoding rules.
pub(crate) fn sync_typed_assignments(
    args: &[String],
    variables: &HashMap<String, String>,
    store: &mut VariableStore,
) {
    let arrays = marked_vars(variables, ARRAY_VARS);
    let assocs = marked_vars(variables, ASSOC_VARS);
    let mut parse_options = true;
    for arg in args {
        if parse_options && arg == "--" {
            parse_options = false;
            continue;
        }
        let Some((raw_name, _)) = arg.split_once('=') else {
            continue;
        };
        let name = raw_name.strip_suffix('+').unwrap_or(raw_name);
        let Some(base) = declare_base_name(name) else {
            continue;
        };
        let Some(value) = variables.get(base) else {
            continue;
        };
        if arrays.contains(base) {
            let entries = indexed_array_entries(value);
            let values = entries.into_iter().map(|(_, value)| value);
            let _ = store.replace_indexed_array(base, values);
        } else if assocs.contains(base) {
            let _ = store.replace_associative_array(base, parse_assoc_words(value));
        }
    }
}

/// Synchronize the final declare attribute markers into the typed variable owner.
pub(crate) fn sync_typed_attributes(
    args: &[String],
    variables: &HashMap<String, String>,
    store: &mut VariableStore,
) {
    let exported = marked_vars(variables, EXPORTED_VARS);
    let readonly = marked_vars(variables, READONLY_VARS);
    let arrays = marked_vars(variables, ARRAY_VARS);
    let assocs = marked_vars(variables, ASSOC_VARS);
    let integer = marked_vars(variables, INTEGER_VARS);
    let uppercase = marked_vars(variables, UPPERCASE_VARS);
    let lowercase = marked_vars(variables, LOWERCASE_VARS);
    let namerefs = marked_vars(variables, NAMEREF_VARS);

    for arg in args {
        let raw_name = arg.split_once('=').map(|(name, _)| name).unwrap_or(arg);
        let name = raw_name.strip_suffix('+').unwrap_or(raw_name);
        let Some(base) = declare_base_name(name) else {
            continue;
        };
        if store.get(base).is_none() {
            let value = variables.get(base).cloned().unwrap_or_default();
            let _ = store.set_scalar(base, value);
        }
        if arrays.contains(base)
            && !matches!(
                store.get(base).map(|v| &v.value),
                Some(crate::shell::ShellValue::IndexedArray(_))
            )
        {
            let _ = store.replace_indexed_array(base, std::iter::empty::<String>());
        } else if assocs.contains(base)
            && !matches!(
                store.get(base).map(|v| &v.value),
                Some(crate::shell::ShellValue::AssociativeArray(_))
            )
        {
            let _ = store.replace_associative_array(base, std::iter::empty::<(String, String)>());
        }
        if let Some(variable) = store.get_mut(base) {
            variable.exported = exported.contains(base);
            variable.readonly = readonly.contains(base);
            variable.integer = integer.contains(base);
            variable.uppercase = uppercase.contains(base);
            variable.lowercase = lowercase.contains(base);
            variable.nameref = if namerefs.contains(base) {
                arg.split_once('=').map(|(_, value)| value.to_string())
            } else {
                None
            };
        }
    }
}

pub fn execute(args: &[String], variables: &mut HashMap<String, String>) -> io::Result<i32> {
    let mut stdout = crate::executor::GlobalStdout;
    let mut stderr = io::stderr();
    execute_with_io(args, variables, &mut stdout, &mut stderr)
}

pub(crate) fn execute_with_io<W, E>(
    args: &[String],
    variables: &mut HashMap<String, String>,
    stdout: &mut W,
    stderr: &mut E,
) -> io::Result<i32>
where
    W: Write,
    E: Write,
{
    execute_with_io_named("declare", args, variables, stdout, stderr)
}

pub(crate) fn execute_with_io_named<W, E>(
    command_name: &str,
    args: &[String],
    variables: &mut HashMap<String, String>,
    stdout: &mut W,
    stderr: &mut E,
) -> io::Result<i32>
where
    W: Write,
    E: Write,
{
    let mut print = false;
    let mut export = false;
    let mut array = false;
    let mut assoc = false;
    let mut integer = false;
    let mut uppercase = false;
    let mut lowercase = false;
    let mut nameref = false;
    let mut readonly = false;
    let mut unset_export = false;
    let mut unset_array = false;
    let mut unset_assoc = false;
    let mut unset_integer = false;
    let mut unset_uppercase = false;
    let mut unset_lowercase = false;
    let mut unset_nameref = false;
    let mut unset_readonly = false;
    let mut names = Vec::new();

    let mut parse_options = true;
    let mut saw_option = false;
    for arg in args {
        if parse_options && arg == "--" {
            parse_options = false;
            continue;
        }
        if parse_options
            && (arg.starts_with('-') || arg.starts_with('+'))
            && arg != "-"
            && arg != "+"
        {
            let set_attr = arg.starts_with('-');
            saw_option = true;
            for option in arg[1..].chars() {
                match option {
                    'p' => print = true,
                    'x' if set_attr => export = true,
                    'x' => unset_export = true,
                    'a' if set_attr => array = true,
                    'a' => unset_array = true,
                    'A' if set_attr => assoc = true,
                    'A' => unset_assoc = true,
                    'i' if set_attr => integer = true,
                    'i' => unset_integer = true,
                    'u' => {
                        if set_attr {
                            uppercase = true;
                            lowercase = false;
                        } else {
                            unset_uppercase = true;
                        }
                    }
                    'l' => {
                        if set_attr {
                            lowercase = true;
                            uppercase = false;
                        } else {
                            unset_lowercase = true;
                        }
                    }
                    'n' if set_attr => nameref = true,
                    'n' => unset_nameref = true,
                    'r' if set_attr => readonly = true,
                    'r' => unset_readonly = true,
                    'g' | 'G' | 'I' => {
                        // TODO(variables.c/builtins/declare.def): `-g` forces
                        // global scope inside functions. Rubash has one
                        // variable table for now. `-I` is a local inheritance
                        // attribute; outside local it is accepted but does not
                        // add a printable variable attribute.
                    }
                    _ => {
                        writeln!(
                            stderr,
                            "{}{command_name}: -{option}: invalid option",
                            diagnostic_prefix(),
                        )?;
                        print_declare_usage(command_name, stderr)?;
                        return Ok(EX_USAGE);
                    }
                }
            }
        } else {
            names.push(arg.as_str());
        }
    }

    let had_name_args = !names.is_empty();
    let mut assign_names = Vec::new();
    let mut attr_status = EXECUTION_SUCCESS;
    for name in &names {
        if !valid_declare_name(name) {
            writeln!(
                stderr,
                "{}declare: `{}`: not a valid identifier",
                diagnostic_prefix(),
                name
            )?;
            attr_status = EXECUTION_FAILURE;
            continue;
        }
        if nameref && nameref_self_reference(name) {
            let name = name.split_once('=').map(|(name, _)| name).unwrap_or(name);
            writeln!(
                stderr,
                "{}declare: {}: nameref variable self references not allowed",
                diagnostic_prefix(),
                name
            )?;
            attr_status = EXECUTION_FAILURE;
            continue;
        }
        assign_names.push(*name);
    }
    let arrays = marked_vars(variables, ARRAY_VARS);
    let assocs = marked_vars(variables, ASSOC_VARS);
    let mut valid_assign_names = Vec::new();
    for name in assign_names {
        let Some(var_name) = declare_base_name(name) else {
            valid_assign_names.push(name);
            continue;
        };
        if assoc && arrays.contains(var_name) && !assocs.contains(var_name) {
            writeln!(
                stderr,
                "{}declare: {}: cannot convert indexed to associative array",
                diagnostic_prefix(),
                var_name
            )?;
            attr_status = EXECUTION_FAILURE;
            continue;
        }
        if array && assocs.contains(var_name) && !arrays.contains(var_name) {
            writeln!(
                stderr,
                "{}declare: {}: cannot convert associative to indexed array",
                diagnostic_prefix(),
                var_name
            )?;
            variables.insert(var_name.to_string(), String::new());
            attr_status = EXECUTION_FAILURE;
            continue;
        }
        valid_assign_names.push(name);
    }
    let assign_names = valid_assign_names;
    if assign_declare_names(
        &assign_names,
        variables,
        array,
        assoc,
        integer,
        !print,
        stderr,
    )? != EXECUTION_SUCCESS
    {
        attr_status = EXECUTION_FAILURE;
    }
    let names = assign_names;
    let options = DeclareOptions {
        export,
        array,
        assoc,
        integer,
        uppercase,
        lowercase,
        nameref,
        readonly,
        unset_export,
        unset_array,
        unset_assoc,
        unset_integer,
        unset_uppercase,
        unset_lowercase,
        unset_nameref,
        unset_readonly,
    };
    attr_status = apply_declare_attrs(&names, variables, options, attr_status, stderr)?;

    let plain = names.is_empty() && !had_name_args && !print && !saw_option;
    if names.is_empty() && !had_name_args {
        print = true;
    }

    if !print {
        return Ok(attr_status);
    }

    print::print_declare_names(
        &names,
        variables,
        options,
        plain,
        attr_status,
        stdout,
        stderr,
    )
}

fn print_declare_usage<W>(command_name: &str, stderr: &mut W) -> io::Result<()>
where
    W: Write,
{
    if command_name == "typeset" {
        writeln!(
            stderr,
            "typeset: usage: typeset [-aAfFgiIlnrtux] name[=value] ... or typeset -p [-aAfFilnrtux] [name ...]"
        )
    } else {
        writeln!(
            stderr,
            "declare: usage: declare [-aAfFgiIlnrtux] [name[=value] ...] or declare -p [-aAfFilnrtux] [name ...]"
        )
    }
}

#[cfg(test)]
#[path = "declare_tests.rs"]
mod tests;
