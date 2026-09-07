//! GNU parse.y skip_double_quoted: single quotes inside double quotes are
//! ordinary literal characters — only ", backslash, dollar and backtick are
//! special there. Rubash used to re-read the surviving quote as a
//! single-quote delimiter at expansion, dropping the quote characters and
//! suppressing parameter expansion across the pseudo span
//! (busybox ash-getopts/local1 differential rows; GNU bash 5.2.21 baseline).

use std::process::Command;

fn rubash(script: &str) -> String {
    let output = Command::new(env!("CARGO_BIN_EXE_rubash"))
        .arg("-c")
        .arg(script)
        .output()
        .expect("run rubash");
    String::from_utf8_lossy(&output.stdout).into_owned()
}

#[test]
fn single_quote_inside_double_quotes_keeps_literal_and_expansion() {
    assert_eq!(
        rubash("v=VAL\necho \"A1:'$v' tail:$v\""),
        "A1:'VAL' tail:VAL\n"
    );
}

#[test]
fn unpaired_single_quote_inside_double_quotes_stays_data() {
    assert_eq!(rubash("echo \"A3:x'y\""), "A3:x'y\n");
}

#[test]
fn embedded_apostrophe_keeps_expansion() {
    assert_eq!(rubash("v=VAL\necho \"it's $v\""), "it's VAL\n");
}

#[test]
fn escaped_single_quote_outside_double_quotes_unchanged() {
    assert_eq!(rubash("echo a\\'b"), "a'b\n");
}
