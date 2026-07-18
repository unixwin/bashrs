//! `printf` builtin.
//!
//! GNU Bash source ownership:
//! - builtins/printf.def (`printf_builtin`)

use std::collections::{BTreeMap, HashMap};
use std::io::{self, Write};

mod escape;
mod float;
mod identifier;
mod number;
mod spec;
pub(crate) mod time;
mod value;

use escape::expand_format_escape;
use identifier::valid_identifier;
use spec::{parse_format_spec, resolve_dynamic_format_args, valid_format_specifier};
use time::format_time_value;
use value::format_value;

const EXECUTION_SUCCESS: i32 = 0;
const EXECUTION_FAILURE: i32 = 1;
const EX_USAGE: i32 = 2;

#[derive(Debug, Clone, Default)]
struct FormatSpec {
    raw: String,
    left_adjust: bool,
    zero_pad: bool,
    alternate_form: bool,
    explicit_sign: bool,
    leading_space_sign: bool,
    width: Option<usize>,
    width_from_arg: bool,
    precision: Option<usize>,
    precision_from_arg: bool,
    time_format: Option<String>,
    specifier: char,
}

#[derive(Debug, Clone)]
struct RenderedPrintf {
    output: String,
    status: i32,
    errors: Vec<String>,
    stop_output: bool,
}

enum ParsedFormat {
    Spec(FormatSpec),
    Missing(String),
}

struct ParsedNumber<T> {
    value: T,
    invalid: Option<String>,
}

/// Execute `printf` with arguments after the command name.
pub fn execute(args: &[String], env_vars: &mut HashMap<String, String>) -> io::Result<i32> {
    let mut stdout = io::stdout().lock();
    let mut stderr = io::stderr().lock();
    execute_with_io(
        args.iter().map(String::as_str),
        env_vars,
        &mut stdout,
        &mut stderr,
    )
}

pub(crate) fn execute_with_io<'a, I, W, E>(
    args: I,
    env_vars: &mut HashMap<String, String>,
    stdout: &mut W,
    stderr: &mut E,
) -> io::Result<i32>
where
    I: IntoIterator<Item = &'a str>,
    W: Write,
    E: Write,
{
    let args: Vec<&str> = args.into_iter().collect();
    let mut output_var = None;
    let mut index = 0;

    let mut end_options = false;
    if args.get(index) == Some(&"--") {
        index += 1;
        end_options = true;
    }

    if !end_options
        && matches!(args.get(index), Some(option) if option.starts_with('-') && !option.starts_with("-v"))
    {
        writeln!(stderr, "rubash: printf: {}: invalid option", args[index])?;
        writeln!(stderr, "printf: usage: printf [-v var] format [arguments]")?;
        return Ok(EX_USAGE);
    }

    if !end_options {
        let name = match args.get(index) {
            Some(&"-v") => {
                let Some(name) = args.get(index + 1) else {
                    writeln!(stderr, "rubash: printf: -v: option requires an argument")?;
                    return Ok(EX_USAGE);
                };
                index += 2;
                Some(*name)
            }
            Some(option) => option
                .strip_prefix("-v")
                .filter(|name| !name.is_empty())
                .map(|name| {
                    index += 1;
                    name
                }),
            None => None,
        };

        if let Some(name) = name {
            if !valid_identifier(name) && !valid_printf_array_target(name, env_vars) {
                writeln!(stderr, "rubash: printf: `{}`: not a valid identifier", name)?;
                return Ok(EX_USAGE);
            }

            output_var = Some(name);
            if args.get(index) == Some(&"--") {
                index += 1;
                end_options = true;
            }
        }
    }

    if !end_options && matches!(args.get(index), Some(option) if option.starts_with('-')) {
        writeln!(stderr, "rubash: printf: {}: invalid option", args[index])?;
        writeln!(stderr, "printf: usage: printf [-v var] format [arguments]")?;
        return Ok(EX_USAGE);
    }

    let Some(format) = args.get(index) else {
        writeln!(stderr, "printf: usage: printf [-v var] format [arguments]")?;
        return Ok(EX_USAGE);
    };

    let rendered = render(format, &args[index + 1..], env_vars);
    if let Some(name) = output_var {
        assign_printf_output(env_vars, name, rendered.output);
    } else {
        stdout.write_all(rendered.output.as_bytes())?;
    }

    for error in rendered.errors {
        writeln!(stderr, "{error}")?;
    }

    Ok(rendered.status)
}

fn valid_printf_array_target(name: &str, env_vars: &HashMap<String, String>) -> bool {
    let Some((base, subscript)) = parse_printf_array_target(name) else {
        return false;
    };
    if is_marked(env_vars, "__RUBASH_ASSOC_VARS", base) {
        return true;
    }
    resolve_printf_indexed_subscript(env_vars, base, subscript).is_some()
}

fn assign_printf_output(env_vars: &mut HashMap<String, String>, name: &str, output: String) {
    if let Some((base, subscript)) = parse_printf_array_target(name) {
        if is_marked(env_vars, "__RUBASH_ASSOC_VARS", base) {
            assign_printf_assoc_element(env_vars, base, subscript, output);
        } else if let Some(index) = resolve_printf_indexed_subscript(env_vars, base, subscript) {
            assign_printf_indexed_element(env_vars, base, index, output);
        } else {
            env_vars.insert(name.to_string(), output);
        }
        return;
    }

    env_vars.insert(name.to_string(), output);
}

fn parse_printf_array_target(name: &str) -> Option<(&str, &str)> {
    let (base, subscript) = name.split_once('[')?;
    let subscript = subscript.strip_suffix(']')?;
    valid_identifier(base).then_some((base, subscript))
}

fn resolve_printf_indexed_subscript(
    env_vars: &HashMap<String, String>,
    name: &str,
    subscript: &str,
) -> Option<usize> {
    if subscript.is_empty() {
        return None;
    }

    let raw_index = crate::executor::arithmetic::eval_conditional_arith_value(subscript, env_vars)?;
    if raw_index >= 0 {
        return usize::try_from(raw_index).ok();
    }

    let current = env_vars.get(name)?;
    let max_index = indexed_entries(current).keys().next_back().copied()?;
    let resolved = i128::try_from(max_index)
        .ok()?
        .checked_add(1)?
        .checked_add(raw_index)?;
    usize::try_from(resolved).ok()
}

fn assign_printf_indexed_element(
    env_vars: &mut HashMap<String, String>,
    name: &str,
    index: usize,
    output: String,
) {
    let mut entries = env_vars
        .get(name)
        .map(|value| indexed_entries(value))
        .unwrap_or_default();
    entries.insert(index, output);
    env_vars.insert(name.to_string(), format_indexed_storage(entries));
    mark_printf_var(env_vars, "__RUBASH_ARRAY_VARS", name);
}

fn assign_printf_assoc_element(
    env_vars: &mut HashMap<String, String>,
    name: &str,
    key: &str,
    output: String,
) {
    let mut entries = env_vars
        .get(name)
        .map(|value| assoc_entries(value))
        .unwrap_or_default();
    if let Some((_, value)) = entries
        .iter_mut()
        .rev()
        .find(|(entry_key, _)| entry_key == key)
    {
        *value = output;
    } else {
        entries.push((key.to_string(), output));
    }
    env_vars.insert(name.to_string(), format_assoc_storage(entries));
}

fn indexed_entries(value: &str) -> BTreeMap<usize, String> {
    let Some(rendered) = value.strip_prefix('\x1d') else {
        return value
            .strip_prefix('(')
            .and_then(|value| value.strip_suffix(')'))
            .map(split_storage_words)
            .unwrap_or_default()
            .into_iter()
            .enumerate()
            .map(|(index, value)| {
                let value = value
                    .split_once('=')
                    .map(|(_, value)| value)
                    .unwrap_or(&value);
                (index, unquote_storage_value(value))
            })
            .collect();
    };

    let Some(inner) = rendered
        .strip_prefix('(')
        .and_then(|value| value.strip_suffix(')'))
    else {
        return BTreeMap::new();
    };

    split_storage_words(inner)
        .into_iter()
        .filter_map(|part| {
            let (key, value) = part.split_once('=')?;
            let index = key
                .trim_start_matches('[')
                .trim_end_matches(']')
                .parse::<usize>()
                .ok()?;
            Some((index, unquote_storage_value(value)))
        })
        .collect()
}

fn assoc_entries(value: &str) -> Vec<(String, String)> {
    let Some(inner) = value
        .strip_prefix('(')
        .and_then(|value| value.strip_suffix(')'))
    else {
        return Vec::new();
    };

    split_storage_words(inner)
        .into_iter()
        .filter_map(|part| {
            let (key, value) = part.split_once('=')?;
            Some((
                unquote_storage_value(key.trim_start_matches('[').trim_end_matches(']')),
                unquote_storage_value(value),
            ))
        })
        .collect()
}

fn format_indexed_storage(entries: BTreeMap<usize, String>) -> String {
    let rendered = entries
        .into_iter()
        .map(|(index, value)| format!("[{index}]={}", quote_storage_value(&value)))
        .collect::<Vec<_>>()
        .join(" ");
    format!("\x1d({rendered})")
}

fn format_assoc_storage(entries: Vec<(String, String)>) -> String {
    format!(
        "({})",
        entries
            .into_iter()
            .map(|(key, value)| format!(
                "[{}]={}",
                quote_assoc_key(&key),
                quote_storage_value(&value)
            ))
            .collect::<Vec<_>>()
            .join(" ")
    )
}

fn quote_assoc_key(key: &str) -> String {
    if !key.is_empty()
        && !key
            .chars()
            .any(|ch| ch.is_ascii_whitespace() || matches!(ch, '"' | '\\' | ']'))
    {
        return key.to_string();
    }
    quote_storage_value(key)
}

fn quote_storage_value(value: &str) -> String {
    if value.contains(['\n', '\r', '\'']) {
        return format!(
            "$'{}'",
            value
                .replace('\\', "\\\\")
                .replace('\n', "\\n")
                .replace('\r', "\\r")
                .replace('\'', "\\'")
        );
    }

    format!(
        "\"{}\"",
        value
            .replace('\\', "\\\\")
            .replace('"', "\\\"")
            .replace('$', "\\$")
            .replace('`', "\\`")
    )
}

fn unquote_storage_value(value: &str) -> String {
    if let Some(inner) = value
        .strip_prefix("$'")
        .and_then(|value| value.strip_suffix('\''))
    {
        return inner
            .replace("\\n", "\n")
            .replace("\\r", "\r")
            .replace("\\'", "'")
            .replace("\\\\", "\\");
    }

    if let Some(inner) = value
        .strip_prefix('"')
        .and_then(|value| value.strip_suffix('"'))
    {
        return unescape_double_quoted_storage(inner);
    }

    value.to_string()
}

fn unescape_double_quoted_storage(value: &str) -> String {
    let mut output = String::new();
    let mut chars = value.chars();
    while let Some(ch) = chars.next() {
        if ch == '\\' {
            if let Some(escaped) = chars.next() {
                output.push(escaped);
            }
        } else {
            output.push(ch);
        }
    }
    output
}

fn split_storage_words(value: &str) -> Vec<String> {
    let mut parts = Vec::new();
    let mut current = String::new();
    let mut quote = None;
    let mut chars = value.chars().peekable();

    while let Some(ch) = chars.next() {
        match quote {
            Some(quote_ch) => {
                current.push(ch);
                if ch == '\\' {
                    if let Some(next) = chars.next() {
                        current.push(next);
                    }
                } else if ch == quote_ch {
                    quote = None;
                }
            }
            None if ch == '"' || ch == '\'' => {
                quote = Some(ch);
                current.push(ch);
            }
            None if ch.is_whitespace() => {
                if !current.is_empty() {
                    parts.push(std::mem::take(&mut current));
                }
            }
            None => current.push(ch),
        }
    }

    if !current.is_empty() {
        parts.push(current);
    }

    parts
}

fn is_marked(env_vars: &HashMap<String, String>, marker: &str, name: &str) -> bool {
    env_vars
        .get(marker)
        .map(|value| value.split('\x1f').any(|marked| marked == name))
        .unwrap_or(false)
}

fn mark_printf_var(env_vars: &mut HashMap<String, String>, marker: &str, name: &str) {
    if is_marked(env_vars, marker, name) {
        return;
    }
    env_vars
        .entry(marker.to_string())
        .and_modify(|value| {
            if !value.is_empty() {
                value.push('\x1f');
            }
            value.push_str(name);
        })
        .or_insert_with(|| name.to_string());
}

fn render(format: &str, args: &[&str], env_vars: &mut HashMap<String, String>) -> RenderedPrintf {
    let mut output = String::new();
    let mut arg_index = 0;
    let mut errors = Vec::new();

    if args.is_empty() {
        return render_one_pass(format, args, &mut arg_index, output, env_vars);
    }

    while arg_index < args.len() {
        let before_arg = arg_index;
        let rendered = render_one_pass(format, args, &mut arg_index, output, env_vars);
        output = rendered.output;
        errors.extend(rendered.errors);
        if rendered.stop_output {
            return RenderedPrintf {
                output,
                status: status_from_errors(&errors),
                errors,
                stop_output: true,
            };
        }

        if arg_index == before_arg {
            break;
        }
    }

    RenderedPrintf {
        output,
        status: status_from_errors(&errors),
        errors,
        stop_output: false,
    }
}

fn render_one_pass(
    format: &str,
    args: &[&str],
    arg_index: &mut usize,
    mut output: String,
    env_vars: &mut HashMap<String, String>,
) -> RenderedPrintf {
    let mut chars = format.chars().peekable();
    let mut errors = Vec::new();

    while let Some(ch) = chars.next() {
        match ch {
            '\\' => output.push_str(&expand_format_escape(&mut chars)),
            '%' => {
                if chars.peek() == Some(&'%') {
                    chars.next();
                    output.push('%');
                    continue;
                }

                let mut spec = match parse_format_spec(&mut chars) {
                    ParsedFormat::Spec(spec) => spec,
                    ParsedFormat::Missing(format) => {
                        return RenderedPrintf {
                            output,
                            status: EXECUTION_FAILURE,
                            errors: vec![format!(
                                "rubash: printf: `{format}': missing format character"
                            )],
                            stop_output: true,
                        };
                    }
                };

                if spec.time_format.is_some() && spec.specifier != 'T' {
                    errors.push(format!(
                        "rubash: printf: warning: `{}': invalid time format specification",
                        spec.specifier
                    ));
                    output.push_str(&spec.raw);
                    continue;
                }

                if !valid_format_specifier(spec.specifier) {
                    return RenderedPrintf {
                        output,
                        status: EXECUTION_FAILURE,
                        errors: vec![format!(
                            "rubash: printf: `{}': invalid format character",
                            spec.specifier
                        )],
                        stop_output: true,
                    };
                };
                errors.extend(resolve_dynamic_format_args(&mut spec, args, arg_index));

                if spec.specifier == 'n' {
                    let name = next_arg(args, arg_index);
                    if valid_identifier(name) {
                        env_vars.insert(name.to_string(), output.chars().count().to_string());
                    }
                } else if spec.specifier == 'T' {
                    let value = if *arg_index < args.len() {
                        next_arg(args, arg_index)
                    } else {
                        "-1"
                    };
                    let (rendered, error) = format_time_value(value, &spec, env_vars);
                    if let Some(error) = error {
                        errors.push(error);
                    }
                    output.push_str(&rendered);
                } else {
                    let value = next_arg(args, arg_index);
                    let (rendered, stop_output, error) = format_value(value, &spec);
                    if let Some(error) = error {
                        errors.push(error);
                    }
                    output.push_str(&rendered);
                    if stop_output {
                        return RenderedPrintf {
                            output,
                            status: status_from_errors(&errors),
                            errors,
                            stop_output: true,
                        };
                    }
                }
            }
            other => output.push(other),
        }
    }
    RenderedPrintf {
        output,
        status: status_from_errors(&errors),
        errors,
        stop_output: false,
    }
}

fn status_from_errors(errors: &[String]) -> i32 {
    if errors.is_empty() {
        EXECUTION_SUCCESS
    } else {
        EXECUTION_FAILURE
    }
}

fn next_arg<'a>(args: &'a [&str], arg_index: &mut usize) -> &'a str {
    let value = args.get(*arg_index).copied().unwrap_or("");
    *arg_index += 1;
    value
}

#[cfg(test)]
#[path = "printf_tests.rs"]
mod tests;
