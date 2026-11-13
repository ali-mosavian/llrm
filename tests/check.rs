//! tests/check: one file in, CHECK lines out, lit and FileCheck style.
//!
//! A file's `RUN:` line names a tool (`llrm-cc`: the C driver, in llrm-c's
//! argument dialect) and its arguments, `%s` being the file (`not` first: it
//! must fail). The tool's stdout and stderr are matched against the file's
//! directives, in a comment (`;`, `'` or `//`):
//!
//!   CHECK: text            a later line contains it
//!   CHECK-LABEL: text      the same, naming where a block starts
//!   CHECK-NEXT: text       the line right after the last match contains it
//!   CHECK-NOT: text        no line between the last match and the next one
//! contains it
//!   CHECK-MAX-N: text      at most N lines between them contain it (a ceiling
//! a known gap is held to; CHECK-NOT is CHECK-MAX-0)
//!
//! `{{regex}}` inside a pattern is a regular expression; runs of blanks match
//! each other.
//!
//! `RUN-PENDING:` is a RUN line not yet run: a gap llrm-cc has to close first.
//!
//! A `RUN:` line may name several configurations: each `{a | b | c}` in it is
//! every alternative, and the product of its groups is run, each against the
//! same directives. Equal groups choose together (`--dialect {qb45 | pds71}
//! --runtime {qb45 | pds71}` is two runs). A directive may be
//! tagged `CHECK[-Os]:` to hold only of the runs whose line contains `-Os`.

use std::path::{Path, PathBuf};
use std::process::Command;

use regex::Regex;

mod common;

#[derive(Debug, PartialEq)]
enum Kind {
    Check,
    Next,
    Not,
}

#[derive(Debug)]
struct Directive {
    /// The text a run's line must contain for this to hold of it; none, of
    /// every run.
    only: Option<String>,
    kind: Kind,
    /// For `Kind::Not`: how many lines of the span may contain the pattern.
    most: usize,
    pattern: String,
    at: usize,
}

fn comment(line: &str) -> Option<&str> {
    let line = line.trim_start();
    [";", "'", "//", "#"].iter().find_map(|mark| line.strip_prefix(mark)).map(str::trim)
}

fn directives(source: &str) -> Vec<Directive> {
    let tagged = Regex::new(r"^CHECK(-LABEL|-NEXT|-NOT|-MAX-[0-9]+)?(?:\[([^\]]*)\])?:(.*)$").expect("a regex");
    let mut found = Vec::new();
    for (at, line) in source.lines().enumerate() {
        let Some(text) = comment(line) else { continue };
        if let Some(parts) = tagged.captures(text) {
            let tag = parts.get(1).map(|one| one.as_str());
            let kind = match tag {
                Some("-NEXT") => Kind::Next,
                Some("-NOT") => Kind::Not,
                Some(max) if max.starts_with("-MAX-") => Kind::Not,
                _ => Kind::Check,
            };
            let most = tag.and_then(|max| max.strip_prefix("-MAX-")).map_or(0, |n| n.parse().expect("a count"));
            found.push(Directive {
                only: parts.get(2).map(|one| one.as_str().to_owned()),
                kind,
                most,
                pattern: parts[3].trim().to_owned(),
                at: at + 1,
            });
        }
    }
    found
}

fn run_lines(source: &str) -> Vec<String> {
    source
        .lines()
        .filter_map(comment)
        .filter_map(|text| text.strip_prefix("RUN:"))
        .flat_map(|rest| expanded(rest.trim()))
        .collect()
}

/// `line` once for each choice among its `{a | b}` groups; equal groups choose
/// alike.
fn expanded(line: &str) -> Vec<String> {
    let group = Regex::new(r"\{([^{}|]*(?:\|[^{}|]*)+)\}").expect("a regex");
    let mut distinct: Vec<(String, Vec<String>)> = Vec::new();
    for found in group.captures_iter(line) {
        if !distinct.iter().any(|(text, _)| *text == found[0]) {
            distinct.push((found[0].to_owned(), found[1].split('|').map(|one| one.trim().to_owned()).collect()));
        }
    }
    let mut lines = vec![line.to_owned()];
    for (text, choices) in distinct {
        lines = lines
            .iter()
            .flat_map(|one| choices.iter().map(|choice| one.replace(&text, choice)).collect::<Vec<_>>())
            .collect();
    }
    lines
}

/// The pattern as a regex: literal text escaped, `{{..}}` kept, blanks any run
/// of blanks.
fn matcher(pattern: &str) -> Regex {
    let mut out = String::new();
    let mut rest = pattern;
    while !rest.is_empty() {
        if let Some(inner) = rest.strip_prefix("{{") {
            let end = inner.find("}}").expect("an unclosed {{ in a pattern");
            // Grouped, as FileCheck does: `{{si|di}}` is one alternation, not
            // the end of the pattern.
            out.push_str("(?:");
            out.push_str(&inner[..end]);
            out.push(')');
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

/// A bar crossed: the pattern found more often than the directive allows.
fn barred_message(
    bar: &Directive,
    hits: &[usize],
    lines: &[&str],
) -> String {
    if bar.most == 0 {
        return format!("line {}: CHECK-NOT {:?} found: {}", bar.at, bar.pattern, lines[hits[0]]);
    }
    format!("line {}: CHECK-MAX-{} {:?} found {} times: {}", bar.at, bar.most, bar.pattern, hits.len(), lines[hits[0]])
}

/// What fails, as a message, or Ok.
fn filecheck(
    output: &str,
    wanted: &[Directive],
) -> Result<(), String> {
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
                    return Err(format!(
                        "line {}: {:?} not found{}",
                        one.at,
                        one.pattern,
                        if one.kind == Kind::Next { " on the next line" } else { "" }
                    ));
                };
                for bar in barred.drain(..) {
                    let re = matcher(&bar.pattern);
                    let hits: Vec<usize> = (cursor..at).filter(|&line| re.is_match(lines[line])).collect();
                    if hits.len() > bar.most {
                        return Err(barred_message(bar, &hits, &lines));
                    }
                }
                last = Some(at);
                cursor = at + 1;
            }
        }
    }
    for bar in barred {
        let re = matcher(&bar.pattern);
        let hits: Vec<usize> = (cursor..lines.len()).filter(|&line| re.is_match(lines[line])).collect();
        if hits.len() > bar.most {
            return Err(barred_message(bar, &hits, &lines));
        }
    }
    Ok(())
}

fn files(
    dir: &Path,
    out: &mut Vec<PathBuf>,
) {
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

/// What a pass over the check files did.
#[derive(Default)]
struct Ran {
    ran: usize,
    failures: Vec<String>,
    /// The toolchain variables an `llrm-cc` RUN line needed and `env` lacked,
    /// with the lines skipped for it.
    skipped: std::collections::BTreeMap<&'static str, usize>,
}

/// Every RUN line of `all`; one that drives `llrm-cc` is skipped, and counted
/// under the variables it lacks, where `env` lacks a toolchain variable it
/// needs. Without that skip each such line failed with the same message: 115
/// false reds on a box without the Open Watcom headers.
fn run_checks(
    root: &Path,
    all: Vec<PathBuf>,
    env: impl Fn(&str) -> Option<std::ffi::OsString>,
    compile: impl Fn(&[String]) -> std::io::Result<std::process::Output>,
) -> Ran {
    let mut done_all = Ran::default();
    // A clang that is not the pin refuses every llrm-cc line alike: one line,
    // and those lines are not run.
    let unpinned = common::cc::unpinned_clangs(&env, common::cc::PIN, |path| common::cc::clang_sha(path));
    done_all.failures.extend(unpinned.iter().cloned());
    for path in all {
        let Ok(source) = std::fs::read_to_string(&path) else { continue };
        for line in run_lines(&source) {
            let mut words: Vec<&str> = line.split_whitespace().collect();
            let must_fail = words.first() == Some(&"not");
            if must_fail {
                words.remove(0);
            }
            let arguments: Vec<String> =
                words[1..].iter().map(|word| word.replace("%s", &path.display().to_string())).collect();
            let done = if words[0] == "llrm-cc" {
                if !unpinned.is_empty() {
                    continue;
                }
                let missing = common::cc::missing_env_for(&arguments, &env);
                if !missing.is_empty() {
                    for name in missing {
                        *done_all.skipped.entry(name).or_default() += 1;
                    }
                    continue;
                }
                // llrm-c's arguments, served by llrm-cc over the pinned clang
                // (needs the Open Watcom tree, LLRM_CLANG_M16).
                compile(&arguments)
            } else {
                Command::new(tool(words[0])).args(&arguments).output()
            }
            .unwrap_or_else(|error| panic!("{}: {}: {error}", path.display(), words[0]));
            let output = format!("{}{}", String::from_utf8_lossy(&done.stdout), String::from_utf8_lossy(&done.stderr));
            let name = path.strip_prefix(root).unwrap().display();
            if done.status.success() == must_fail {
                // The tool's own first line says why: a stale toolchain is one
                // cause, not 114 silent reds.
                let why = output.lines().find(|one| !one.trim().is_empty()).unwrap_or("no output").trim().to_owned();
                done_all
                    .failures
                    .push(format!("{name}: `{line}` {}: {why}", if must_fail { "succeeded" } else { "failed" }));
            } else if let Err(why) = filecheck(&output, &for_run(directives(&source), &line)) {
                done_all.failures.push(format!("{name}: {why}\n{output}"));
            }
            done_all.ran += 1;
        }
    }
    done_all
}

#[test]
fn test_every_file_under_tests_check_satisfies_its_check_lines() {
    let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/check");
    let mut all = Vec::new();
    files(&root, &mut all);
    all.sort();
    let Ran { ran, failures, skipped } =
        run_checks(&root, all, common::cc::toolchain_env, |arguments| common::cc::llrm_c().args(arguments).output());
    println!("{ran} RUN lines");
    if !skipped.is_empty() {
        println!("SKIPPED llrm-cc RUN lines, toolchain missing (lines per variable): {skipped:?}");
    }
    assert!(ran > 0 || !skipped.is_empty(), "premise: some file under tests/check has a RUN line");
    assert!(failures.is_empty(), "{}", grouped(&failures));
}

/// The failures with the same cause once, and how many lines it hit: `name:
/// `line` failed: <why>` lines share a `<why>`.
fn grouped(failures: &[String]) -> String {
    let mut causes: std::collections::BTreeMap<&str, Vec<&str>> = std::collections::BTreeMap::new();
    let mut other = Vec::new();
    for failure in failures {
        match failure.split_once("` failed: ") {
            Some((head, why)) if !why.contains('\n') => {
                causes.entry(why).or_default().push(head.split(':').next().unwrap_or(head))
            }
            _ => other.push(failure.as_str()),
        }
    }
    let mut out: Vec<String> = causes
        .into_iter()
        .map(|(why, names)| {
            format!("{} lines failed alike ({}...): {why}", names.len(), names[..names.len().min(3)].join(", "))
        })
        .collect();
    out.extend(other.iter().map(|one| (*one).to_owned()));
    out.join("\n")
}

/// A box without the Open Watcom headers, the pinned clang or the m16 clang
/// failed every `llrm-cc` RUN line (115 on 2026-11-12): they are skipped, named
/// once, and what does not need the toolchain still runs.
#[test]
fn test_a_missing_toolchain_skips_the_llrm_cc_lines_instead_of_failing_them() {
    let dir = std::env::temp_dir().join(format!("check-skip-{}", std::process::id()));
    std::fs::create_dir_all(&dir).unwrap();
    let file = dir.join("one.c");
    std::fs::write(&file, "// RUN: llrm-cc -m32 -S %s\n// CHECK: nothing\n").unwrap();
    let none = |_: &str| None;
    let result = run_checks(&dir, vec![file], none, |_| unreachable!("a skipped line is not compiled"));
    std::fs::remove_dir_all(&dir).ok();
    assert_eq!(result.ran, 0);
    assert!(result.failures.is_empty(), "{:?}", result.failures);
    assert_eq!(result.skipped.get("LLRM_PIN_CLANG"), Some(&1));
    assert_eq!(result.skipped.get("OW_TREE"), Some(&1));
}

/// The directives that hold of the run `line`.
fn for_run(
    wanted: Vec<Directive>,
    line: &str,
) -> Vec<Directive> {
    wanted.into_iter().filter(|one| one.only.as_ref().is_none_or(|only| line.contains(only.as_str()))).collect()
}

fn check(
    output: &str,
    source: &str,
) -> Result<(), String> {
    filecheck(output, &directives(source))
}

/// `div {{si|di}}` was the regex `div si|di`: it matched any line with "di", so
/// lsr_real_mode_drop's `CHECK-NOT: imul` started ahead of the div and went red
/// on the imul before it.
#[test]
fn test_an_alternation_in_a_pattern_stays_inside_it() {
    assert!(!matcher("div {{si|di}}").is_match("    mov di, 1"));
    assert!(matcher("div {{si|di}}").is_match("    div di"));
    assert!(check("mov di, 1\nimul ax, si, 3\ndiv si\n", "// CHECK: div {{si|di}}\n// CHECK-NOT: imul\n").is_ok());
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

#[test]
fn test_a_run_line_is_each_choice_of_its_groups() {
    assert_eq!(expanded("t %s -O2"), ["t %s -O2"]);
    assert_eq!(expanded("t {-O2 | -Os} %s"), ["t -O2 %s", "t -Os %s"]);
    assert_eq!(
        expanded("t --dialect {a | b} --runtime {a | b} {-O2 | -Os}"),
        [
            "t --dialect a --runtime a -O2",
            "t --dialect a --runtime a -Os",
            "t --dialect b --runtime b -O2",
            "t --dialect b --runtime b -Os"
        ]
    );
}

/// A second configuration that breaks must fail the file, and a directive
/// tagged for the first must not be asked of it.
#[test]
fn test_a_second_configuration_that_breaks_fails_the_file() {
    let source = "// RUN: t {-O2 | -Os}\n// CHECK: rep movsd\n// CHECK[-Os]: pop es\n";
    let runs = run_lines(source);
    assert_eq!(runs.len(), 2);
    let outputs = ["rep movsd\n", "mov ax, 1\n"];
    let results: Vec<_> =
        runs.iter().zip(outputs).map(|(line, out)| filecheck(out, &for_run(directives(source), line))).collect();
    assert!(results[0].is_ok(), "{:?}", results[0]);
    assert!(results[1].is_err(), "the second configuration's output has no rep movsd");
    let tagged = "// CHECK[-Os]: pop es\n";
    assert!(for_run(directives(tagged), &runs[0]).is_empty() && for_run(directives(tagged), &runs[1]).len() == 1);
}

/// A ceiling counts every hit of its span: N pass, N + 1 fail.
#[test]
fn test_check_max_holds_a_known_gap_to_its_ceiling() {
    let source = "; CHECK: a\n; CHECK-MAX-2: x\n; CHECK: c";
    assert!(check("a\nx\nx\nc\n", source).is_ok(), "two are allowed");
    assert!(check("a\nx\nx\nx\nc\n", source).is_err(), "three are one too many");
    assert!(check("a\nx\nx\nx\n", "; CHECK: a\n; CHECK-MAX-2: x").is_err(), "a trailing ceiling covers the rest");
}

/// An unset toolchain variable made `llrm-cc` fall back to the system clang: 69
/// false reds on 2026-11-05.
#[test]
fn test_an_unset_toolchain_variable_is_named() {
    let none = |_: &str| None;
    assert_eq!(common::cc::missing_env(none, false, false), ["LLRM_PIN_CLANG", "OW_TREE", "LLRM_CLANG_M16"]);
    assert_eq!(common::cc::missing_env(none, true, false), ["LLRM_PIN_CLANG", "OW_TREE"]);
    assert_eq!(common::cc::missing_env(none, false, true), ["LLRM_PIN_CLANG"]);
    let set = |_: &str| Some(std::ffi::OsString::from("/"));
    assert!(common::cc::missing_env(set, false, false).is_empty(), "set to an existing path: nothing missing");
    let gone = |_: &str| Some(std::ffi::OsString::from("/no/such/clang"));
    assert_eq!(common::cc::missing_env(gone, false, true), ["LLRM_PIN_CLANG"], "a path that does not exist counts");
}

/// A clang that runs but is not the pin is refused up front, one line naming
/// its version and the pin's.
#[test]
fn test_a_clang_that_is_not_the_pin_is_refused_with_both_versions() {
    let pin = "d1106deb71cc81016406a1917d0508d2df296266\n7e3542bb5338616657772790c7499f26077216b8 # m16\n";
    let set = |_: &str| Some(std::ffi::OsString::from("/x/clang"));
    let at = |sha: &'static str| move |_: &std::ffi::OsStr| Some(sha.to_string());
    let stale = common::cc::unpinned_clangs(set, pin, at("4b787ddb5b7f0000000000000000000000000000"));
    assert_eq!(stale.len(), 2, "{stale:?}");
    assert!(
        stale[1].contains("LLRM_CLANG_M16") && stale[1].contains("4b787ddb5b7f") && stale[1].contains("7e3542bb5338")
    );
    let right = |path: &std::ffi::OsStr| {
        Some(
            if path == "/x/m16" {
                "7e3542bb5338616657772790c7499f26077216b8"
            } else {
                "d1106deb71cc81016406a1917d0508d2df296266"
            }
            .to_string(),
        )
    };
    let both = |name: &str| Some(std::ffi::OsString::from(if name == "LLRM_CLANG_M16" { "/x/m16" } else { "/x/pin" }));
    assert!(common::cc::unpinned_clangs(both, pin, right).is_empty());
    assert!(common::cc::unpinned_clangs(set, pin, |_| None).is_empty(), "one that does not run is missing_env's");
}

/// An unset clang variable left the 113 m16 lines unrun (2026-11-13): it
/// resolves to the build the pin's sha names in the cache, and to nothing when
/// that build is absent or a different sha.
#[test]
fn test_an_unset_clang_resolves_to_the_pinned_build_in_the_cache() {
    let pin = "d1106deb71cc0000000000000000000000000000\n7e3542bb53380000000000000000000000000000 # m16\n";
    let cache = std::env::temp_dir().join(format!("check-cache-{}", std::process::id()));
    let make = |dir: &str| {
        std::fs::create_dir_all(cache.join("llrm").join(dir).join("bin")).unwrap();
        std::fs::write(cache.join("llrm").join(dir).join("bin/clang"), "").unwrap();
    };
    make("clang-m16-4b787ddb5b7f");
    assert_eq!(common::cc::pinned_clang("LLRM_CLANG_M16", pin, &cache), None, "a stale sha is not the pin");
    make("clang-m16-7e3542bb5338");
    make("clang-d1106deb71cc");
    let m16 = common::cc::pinned_clang("LLRM_CLANG_M16", pin, &cache).unwrap();
    assert!(m16.ends_with("clang-m16-7e3542bb5338/bin/clang"), "{m16:?}");
    assert!(common::cc::pinned_clang("LLRM_PIN_CLANG", pin, &cache).unwrap().ends_with("clang-d1106deb71cc/bin/clang"));
    assert_eq!(common::cc::pinned_clang("OW_TREE", pin, &cache), None);
    std::fs::remove_dir_all(&cache).ok();
}

/// A stale cached m16 clang made `llrm-cc` refuse every line with "is not the
/// pinned clang" on its stderr, which the harness dropped: 114 reds that all
/// said "failed" (2026-11-12). The tool's first line is in the failure, and one
/// cause is one line of the report.
#[test]
fn test_one_toolchain_fault_is_one_line_with_its_message() {
    let dir = std::env::temp_dir().join(format!("check-stale-{}", std::process::id()));
    std::fs::create_dir_all(&dir).unwrap();
    let files: Vec<PathBuf> = (0..3)
        .map(|n| {
            let file = dir.join(format!("f{n}.c"));
            std::fs::write(&file, "// RUN: llrm-cc -m32 -S %s\n// CHECK: x\n").unwrap();
            file
        })
        .collect();
    let set = |_: &str| Some(std::ffi::OsString::from("/"));
    let stale = |_: &[String]| {
        use std::os::unix::process::ExitStatusExt;
        Ok(std::process::Output {
            status: std::process::ExitStatus::from_raw(1 << 8),
            stdout: Vec::new(),
            stderr: b"llrm-cc: /x/clang is not the pinned clang d1106deb\n".to_vec(),
        })
    };
    let result = run_checks(&dir, files, set, stale);
    std::fs::remove_dir_all(&dir).ok();
    assert_eq!(result.failures.len(), 3);
    let report = grouped(&result.failures);
    assert_eq!(report.lines().count(), 1, "{report}");
    assert!(report.contains("3 lines failed alike") && report.contains("is not the pinned clang"), "{report}");
}
