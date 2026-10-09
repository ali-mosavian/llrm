//! `rfmt-post --stdin` is the filter fmt.sh and editors pipe a file's text
//! through.

use std::io::Write;
use std::process::{Command, Stdio};

fn filter(
    args: &[&str],
    input: &str,
) -> (bool, String) {
    let mut child = Command::new(env!("CARGO_BIN_EXE_rfmt-post"))
        .args(args)
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::null())
        .spawn()
        .unwrap();
    child.stdin.take().unwrap().write_all(input.as_bytes()).unwrap();
    let out = child.wait_with_output().unwrap();
    (out.status.success(), String::from_utf8(out.stdout).unwrap())
}

fn read(name: &str) -> String {
    std::fs::read_to_string(format!("{}/fixtures/{name}", env!("CARGO_MANIFEST_DIR"))).unwrap()
}

/// Without the flag the argument was taken for a file name and nothing came out
/// of stdout.
#[test]
fn stdin_splits_chains_like_the_library() {
    let (ok, out) = filter(&["--stdin"], &read("example.in.rs"));
    assert!(ok);
    assert_eq!(out, rfmt_post::format(&read("example.in.rs")).unwrap().0);
}

#[test]
fn stdin_with_pre_runs_only_the_steps_before_rustfmt() {
    let (ok, out) = filter(&["--pre", "--stdin"], &read("matches.in.rs"));
    assert!(ok);
    assert_eq!(out, rfmt_post::pre(&read("matches.in.rs")).unwrap().0);
}

/// An editor must keep the buffer: a file that does not parse is an error and
/// no output.
#[test]
fn stdin_that_does_not_parse_fails_and_writes_nothing() {
    let (ok, out) = filter(&["--stdin"], "fn (((");
    assert!(!ok);
    assert!(out.is_empty());
}

const LONG: &str = "a comment far longer than eighty columns, which no line break can bring under it unless it moves";

/// The check named a comment line nothing could fix by its number; the exempt
/// ones stay out of the list.
#[test]
fn long_comments_lists_the_ones_left_past_column_80() {
    let after_a_string = format!("fn f() -> &'static str {{\n    \"a\nb\" // {LONG}\n}}\n");
    let (ok, out) = filter(&["--long-comments", "--stdin"], &after_a_string);
    assert!(!ok);
    assert_eq!(out, "3:Other\n");
    let trailing = format!("fn f() {{\n    g(); // {LONG}\n}}\n");
    let (ok, out) = filter(&["--long-comments", "--stdin"], &trailing);
    assert!(!ok);
    assert_eq!(out, "2:Trailing\n");
}

/// A word past the width, a code block and a table row cannot be broken; with
/// `--all` they are listed and pass.
#[test]
fn long_comments_passes_a_word_a_code_block_and_a_table_row() {
    let (ok, out) = filter(&["--long-comments", "--stdin"], &read("comment_exempt.in.rs"));
    assert!(ok);
    assert!(out.is_empty());
    let (ok, out) = filter(&["--long-comments", "--all", "--stdin"], &read("comment_exempt.in.rs"));
    assert!(ok);
    assert_eq!(out, "2:Unbreakable\n5:CodeBlock\n8:Table\n");
}

/// The formatted output has none left: a pass that missed a kind of comment
/// shows here, not in the gate.
#[test]
fn long_comments_finds_nothing_in_what_the_formatter_made_of_the_fixtures() {
    for name in ["comment_wrap", "comment_exempt"] {
        let out = rfmt_post::scan(&read(&format!("{name}.out.rs"))).unwrap();
        assert!(out.iter().all(|long| long.kind.exempt()), "{name}");
    }
}
