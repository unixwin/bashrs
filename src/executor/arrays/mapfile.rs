pub(in crate::executor) fn split_mapfile_input(
    input: &str,
    delimiter: Option<char>,
    trim_delimiter: bool,
) -> Vec<String> {
    let Some(delimiter) = delimiter else {
        return input
            .split_inclusive('\n')
            .map(|line| {
                if trim_delimiter {
                    line.trim_end_matches('\n').to_string()
                } else {
                    line.to_string()
                }
            })
            .collect();
    };

    let mut values = Vec::new();
    let mut current = String::new();
    for ch in input.chars() {
        current.push(ch);
        if ch == delimiter {
            // Bash treats NUL as a delimiter even without -t because a NUL
            // cannot be retained in a shell variable's value.
            if trim_delimiter || delimiter == '\0' {
                current.pop();
            }
            values.push(std::mem::take(&mut current));
        }
    }
    if !current.is_empty() {
        values.push(current);
    }
    values
}
