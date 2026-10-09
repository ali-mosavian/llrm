//! `rfmt-post --stdin` is the filter fmt.sh and editors pipe a file's text through.

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

/// Without the flag the argument was taken for a file name and nothing came out of stdout.
#[test]
fn stdin_splits_chains_like_the_library() {
    let (ok, out) = filter(&["--stdin"], &read("example.in.rs"));
    assert!(ok);
    assert_eq!(out, rfmt_post::format(&read("example.in.rs")).unwrap().0);
}

#[test]
fn stdin_with_matches_breaks_only_matches() {
    let (ok, out) = filter(&["--matches", "--stdin"], &read("matches.in.rs"));
    assert!(ok);
    assert_eq!(out, rfmt_post::matches_only(&read("matches.in.rs")).unwrap().0);
}

/// An editor must keep the buffer: a file that does not parse is an error and no output.
#[test]
fn stdin_that_does_not_parse_fails_and_writes_nothing() {
    let (ok, out) = filter(&["--stdin"], "fn (((");
    assert!(!ok);
    assert!(out.is_empty());
}
