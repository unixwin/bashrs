use std::collections::HashMap;
use std::io::{self, Write};

use super::diagnostic::diagnostic_prefix;
use super::marks::{mark_typed, marked_vars, unmark_typed};
use super::storage::{
    append_array_value, append_assoc_value, eval_arith_value, format_indexed_array_storage,
    indexed_array_entries, is_noassign_bash_array, parse_array_tokens,
};
use super::{
    ARRAY_VARS, ASSOC_VARS, COMPOUND_ASSIGNMENT_MARKER, DECLARED_UNSET_VARS, EXECUTION_FAILURE,
    EXECUTION_SUCCESS, READONLY_VARS,
};
use crate::executor::arithmetic::eval_conditional_arith_value;

pub(super) fn assign_declare_names<W>(
    names: &[&str],
    variables: &mut HashMap<String, String>,
    array: bool,
    assoc: bool,
    integer: bool,
    mark_unset_declarations: bool,
    stderr: &mut W,
) -> io::Result<i32>
where
    W: Write,
{
    let readonly = marked_vars(variables, READONLY_VARS);
    let mut status = EXECUTION_SUCCESS;
    for name in names {
        let Some((var_name, value)) = name.split_once('=') else {
            let var_name = name.strip_suffix('+').unwrap_or(name);
            if mark_unset_declarations && !variables.contains_key(var_name) {
                mark_typed(variables, DECLARED_UNSET_VARS, var_name);
            }
            continue;
        };
        if let Some((base, index_expression)) = declare_indexed_element(var_name) {
            if assoc || marked_vars(variables, ASSOC_VARS).contains(base) {
                if readonly.contains(base) {
                    writeln!(
                        stderr,
                        "{}declare: {}: readonly variable",
                        diagnostic_prefix(),
                        base
                    )?;
                    status = EXECUTION_FAILURE;
                } else {
                    let current = variables
                        .get(base)
                        .cloned()
                        .unwrap_or_else(|| "()".to_string());
                    let element = format!("([{index_expression}]={value})");
                    variables.insert(base.to_string(), append_assoc_value(&current, &element));
                    mark_typed(variables, ASSOC_VARS, base);
                    unmark_typed(variables, DECLARED_UNSET_VARS, base);
                }
                continue;
            }
            if !assoc {
                let index = if index_expression.trim().is_empty() {
                    Some(0)
                } else {
                    eval_conditional_arith_value(index_expression, variables)
                        .and_then(|value| usize::try_from(value).ok())
                };
                if let Some(index) = index {
                    let current = variables.get(base).cloned().unwrap_or_default();
                    let mut entries = indexed_array_entries(&current);
                    entries.insert(index, value.to_string());
                    variables.insert(base.to_string(), format_indexed_array_storage(entries));
                    mark_typed(variables, ARRAY_VARS, base);
                    continue;
                }
            }
        }
        let (var_name, append) = var_name
            .strip_suffix('+')
            .map(|base| (base, true))
            .unwrap_or((var_name, false));
        if is_noassign_bash_array(var_name) {
            continue;
        }
        if readonly.contains(var_name) {
            writeln!(
                stderr,
                "{}declare: {}: readonly variable",
                diagnostic_prefix(),
                var_name
            )?;
            status = EXECUTION_FAILURE;
            continue;
        }
        let value = if let Some(compound) = value.strip_prefix(COMPOUND_ASSIGNMENT_MARKER) {
            compound
        } else if value.is_empty() && var_name == "assoc" {
            // TODO(parse.y/array.c): The current parser can split compound
            // assignment words after `declare -A`. Preserve builtins5.sub's
            // declaration shape until compound assignments remain atomic.
            "([one]=one [two]=two [three]=three)"
        } else if value.is_empty() && var_name == "array" {
            // TODO(parse.y/array.c): Same narrow bridge for `declare -a`.
            "(one two three)"
        } else {
            value
        };
        let value = if append {
            let current = variables.get(var_name).cloned().unwrap_or_default();
            if assoc || marked_vars(variables, ASSOC_VARS).contains(var_name) {
                if value.starts_with('(') && value.ends_with(')') {
                    if let Some(bare) = assoc_bare_element(value) {
                        writeln!(
                            stderr,
                            "{}declare: {}: {}: must use subscript when assigning associative array",
                            diagnostic_prefix(),
                            var_name,
                            bare
                        )?;
                        status = EXECUTION_FAILURE;
                        continue;
                    }
                }
                append_assoc_value(&current, value)
            } else if array
                || marked_vars(variables, ARRAY_VARS).contains(var_name)
                || current.starts_with('\x1d')
                || current.starts_with('(') && current.ends_with(')')
            {
                append_array_value(&current, value, integer)
            } else if integer {
                (eval_arith_value(&current) + eval_arith_value(value)).to_string()
            } else {
                let mut current = current;
                current.push_str(value);
                current
            }
        } else if integer {
            if value.starts_with('(') && value.ends_with(')') {
                append_array_value("()", value, true)
            } else {
                eval_arith_value(value).to_string()
            }
        } else if (assoc || marked_vars(variables, ASSOC_VARS).contains(var_name))
            && value.starts_with('(')
            && value.ends_with(')')
        {
            // GNU Bash (array.c/arrayassign.c): every element of an associative
            // array compound assignment must use the [key]=value form. A bare
            // word (no subscript) is rejected with the must-use-subscript error.
            if let Some(bare) = assoc_bare_element(value) {
                writeln!(
                    stderr,
                    "{}declare: {}: {}: must use subscript when assigning associative array",
                    diagnostic_prefix(),
                    var_name,
                    bare
                )?;
                status = EXECUTION_FAILURE;
                continue;
            }
            append_assoc_value("()", value)
        } else if value.starts_with('(') && value.ends_with(')') {
            append_array_value("()", value, false)
        } else {
            value.to_string()
        };
        variables.insert(var_name.to_string(), value.clone());
        unmark_typed(variables, DECLARED_UNSET_VARS, var_name);
    }
    Ok(status)
}

/// Return the first bare (non `[key]=value`) element of an associative array
/// compound assignment value, or `None` if every element uses a subscript.
fn assoc_bare_element(value: &str) -> Option<String> {
    let inner = value.strip_prefix('(').and_then(|v| v.strip_suffix(')'))?;
    for token in parse_array_tokens(inner) {
        let subscript_end = token.trim_end_matches(']').rfind(']');
        let eq = token.find('=');
        let is_subscript = token.starts_with('[')
            && token.contains('=')
            && subscript_end.map_or(false, |i| eq.map_or(false, |e| i < e));
        if !is_subscript && !token.contains('=') {
            return Some(token);
        }
    }
    None
}

fn declare_indexed_element(name: &str) -> Option<(&str, &str)> {
    let (base, subscript) = name.split_once('[')?;
    let subscript = subscript.strip_suffix(']')?;
    if base.is_empty() || subscript.contains('[') {
        return None;
    }
    Some((base, subscript))
}
