//! tests/check: one file in, CHECK lines out, lit and FileCheck style.
//!
//! A file's `RUN:` line names a tool and its arguments, `%s` being the file
//! (`not` first: it must fail). The tool's stdout and stderr are matched against
//! the file's directives, in a comment (`;`, `'` or `//`):
//!
//!   CHECK: text            a later line contains it
//!   CHECK-LABEL: text      the same, naming where a block starts
//!   CHECK-NEXT: text       the line right after the last match contains it
//!   CHECK-NOT: text        no line between the last match and the next one contains it
//!
//! `{{regex}}` inside a pattern is a regular expression; runs of blanks match each other.

use std::path::{Path, PathBuf};
use std::process::Command;

use regex::Regex;

#[derive(Debug, PartialEq)]
enum Kind {
    Check,
    Next,
    Not,
}

#[derive(Debug)]
struct Directive {
    kind: Kind,
    pattern: String,
    at: usize,
}

fn comment(line: &str) -> Option<&str> {
    let line = line.trim_start();
    [";", "'", "//", "#"].iter().find_map(|mark| line.strip_prefix(mark)).map(str::trim)
}

fn directives(source: &str) -> Vec<Directive> {
    let mut found = Vec::new();
    for (at, line) in source.lines().enumerate() {
        let Some(text) = comment(line) else { continue };
        for (name, kind) in [("CHECK-LABEL:", Kind::Check), ("CHECK-NEXT:", Kind::Next), ("CHECK-NOT:", Kind::Not), ("CHECK:", Kind::Check)] {
            if let Some(pattern) = text.strip_prefix(name) {
                found.push(Directive { kind, pattern: pattern.trim().to_owned(), at: at + 1 });
                break;
            }
        }
    }
    found
}

fn run_lines(source: &str) -> Vec<String> {
    source.lines().filter_map(comment).filter_map(|text| text.strip_prefix("RUN:")).map(|rest| rest.trim().to_owned()).collect()
}

/// The pattern as a regex: literal text escaped, `{{..}}` kept, blanks any run of blanks.
fn matcher(pattern: &str) -> Regex {
    let mut out = String::new();
    let mut rest = pattern;
    while !rest.is_empty() {
        if let Some(inner) = rest.strip_prefix("{{") {
            let end = inner.find("}}").expect("an unclosed {{ in a pattern");
            out.push_str(&inner[..end]);
            rest = &inner[end + 2..];
        } else {
            let end = rest.find("{{").unwrap_or(rest.len());
            let chunk = &rest[..end];
            let blank = |at: bool| if at { r"\s+" } else { "" };
            out.push_str(blank(chunk.starts_with(char::is_whitespace)));
            out.push_str(&chunk.split_whitespace().map(regex::escape).collect::<Vec<_>>().join(r"\s+"));
            out.push_str(blank(chunk.ends_with(char::is_whitespace) && !chunk.trim().is_empty()));
            rest = &rest[end..];
        }
    }
    Regex::new(&out).unwrap_or_else(|error| panic!("pattern {pattern:?}: {error}"))
}

/// What fails, as a message, or Ok.
fn filecheck(output: &str, wanted: &[Directive]) -> Result<(), String> {
    let lines: Vec<&str> = output.lines().collect();
    let mut cursor = 0; // the first line a CHECK may match
    let mut last: Option<usize> = None; // the line of the last match
    let mut barred: Vec<&Directive> = Vec::new();
    for one in wanted {
        let re = matcher(&one.pattern);
        match one.kind {
            Kind::Not => barred.push(one),
            Kind::Check | Kind::Next => {
                let found = if one.kind == Kind::Next {
                    let next = last.map_or(0, |at| at + 1);
                    lines.get(next).filter(|line| re.is_match(line)).map(|_| next)
                } else {
                    (cursor..lines.len()).find(|&at| re.is_match(lines[at]))
                };
                let Some(at) = found else {
                    return Err(format!("line {}: {:?} not found{}", one.at, one.pattern, if one.kind == Kind::Next { " on the next line" } else { "" }));
                };
                for bar in barred.drain(..) {
                    let re = matcher(&bar.pattern);
                    if let Some(hit) = (cursor..at).find(|&line| re.is_match(lines[line])) {
                        return Err(format!("line {}: CHECK-NOT {:?} found: {}", bar.at, bar.pattern, lines[hit]));
                    }
                }
                last = Some(at);
                cursor = at + 1;
            }
        }
    }
    for bar in barred {
        let re = matcher(&bar.pattern);
        if let Some(hit) = (cursor..lines.len()).find(|&line| re.is_match(lines[line])) {
            return Err(format!("line {}: CHECK-NOT {:?} found: {}", bar.at, bar.pattern, lines[hit]));
        }
    }
    Ok(())
}

fn files(dir: &Path, out: &mut Vec<PathBuf>) {
    for entry in std::fs::read_dir(dir).unwrap().flatten() {
        let path = entry.path();
        if path.is_dir() {
            files(&path, out);
        } else {
            out.push(path);
        }
    }
}

fn tool(name: &str) -> PathBuf {
    Path::new(env!("CARGO_BIN_EXE_llrm-qb")).with_file_name(name)
}

#[test]
fn test_every_file_under_tests_check_satisfies_its_check_lines() {
    let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/check");
    let mut all = Vec::new();
    files(&root, &mut all);
    all.sort();
    let (mut ran, mut failures) = (0, Vec::new());
    for path in all {
        let Ok(source) = std::fs::read_to_string(&path) else { continue };
        for line in run_lines(&source) {
            let mut words: Vec<&str> = line.split_whitespace().collect();
            let must_fail = words.first() == Some(&"not");
            if must_fail {
                words.remove(0);
            }
            let arguments: Vec<String> = words[1..].iter().map(|word| word.replace("%s", &path.display().to_string())).collect();
            let done = Command::new(tool(words[0])).args(&arguments).output().unwrap_or_else(|error| panic!("{}: {}: {error}", path.display(), words[0]));
            let output = format!("{}{}", String::from_utf8_lossy(&done.stdout), String::from_utf8_lossy(&done.stderr));
            let name = path.strip_prefix(&root).unwrap().display();
            if done.status.success() == must_fail {
                failures.push(format!("{name}: `{line}` {}", if must_fail { "succeeded" } else { "failed" }));
            } else if let Err(why) = filecheck(&output, &directives(&source)) {
                failures.push(format!("{name}: {why}\n{output}"));
            }
            ran += 1;
        }
    }
    println!("{ran} RUN lines");
    assert!(ran > 0, "premise: some file under tests/check has a RUN line");
    assert!(failures.is_empty(), "{}", failures.join("\n"));
}

fn check(output: &str, source: &str) -> Result<(), String> {
    filecheck(output, &directives(source))
}

/// A matcher that accepts everything makes every file pass: it must refuse.
#[test]
fn test_a_missing_check_line_fails() {
    assert!(check("define @a\n", "; CHECK: define @b").is_err());
    assert!(check("define @a\n", "; CHECK: define @a").is_ok());
}

#[test]
fn test_check_next_wants_the_very_next_line() {
    let source = "; CHECK: a\n; CHECK-NEXT: c";
    assert!(check("a\nb\nc\n", source).is_err(), "c is two lines after a");
    assert!(check("a\nc\n", source).is_ok());
}

#[test]
fn test_check_not_bars_only_the_span_before_the_next_match() {
    let source = "; CHECK: a\n; CHECK-NOT: x\n; CHECK: c";
    assert!(check("a\nx\nc\n", source).is_err());
    assert!(check("a\nc\nx\n", source).is_ok(), "x after the last match is outside the span");
    assert!(check("a\nx\n", "; CHECK: a\n; CHECK-NOT: x").is_err(), "a trailing CHECK-NOT covers the rest");
}

#[test]
fn test_patterns_take_regexes_in_braces_and_ignore_blank_runs() {
    assert!(check("  %1 =   add i16 %0, 1\n", "; CHECK: %1 = add i16 {{%[0-9]+}}, 1").is_ok());
    assert!(check("a.b\n", "; CHECK: a.b").is_ok());
    assert!(check("axb\n", "; CHECK: a.b").is_err(), "outside braces a dot is a dot");
}
