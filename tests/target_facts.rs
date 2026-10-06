//! Shared code names no target and holds no copy of a target's facts: they are
//! read from the target's description through `Target`. The facts searched for
//! are taken from the descriptions, so a new target or a changed description
//! changes the search with no list to edit.
//!
//! `target_facts.baseline` holds today's remaining copies per file. It may only
//! shrink: a file over its count, or absent from it, fails; a file under its
//! count fails too, until the baseline is lowered (`LLRM_BLESS=1`). The goal is
//! an empty baseline.

use std::collections::BTreeMap;
use std::fs;
use std::path::{Path, PathBuf};

use regex::Regex;

const ROOT: &str = env!("CARGO_MANIFEST_DIR");
const BASELINE: &str = "tests/target_facts.baseline";
/// Shared code: everything but the target crates, the tests and the build products.
const SHARED: [&str; 8] = [
    "crates/backend",
    "crates/ir",
    "crates/opt",
    "crates/support",
    "crates/bc",
    "crates/frontends",
    "src",
    "tools",
];

/// `a1b2` as a case-insensitive pattern that accepts `_` between digits.
fn number(value: u64) -> String {
    let hex: String = format!("{value:x}").chars().map(|digit| format!("{digit}_?")).collect();
    format!(r"(?:{value}|0x_?{hex}\b)")
}

/// The pattern of every fact the descriptions state that shared code must not.
fn facts() -> Regex {
    let mut parts: Vec<String> = Vec::new();
    for target in llrm_driver::all() {
        let name = target.name();
        parts.push(regex::escape(name));
        parts.push(regex::escape(&name.replace('-', "_")));
        // `code16`, `Code16`, `code16_options`: the part after the family.
        if let Some(variant) = name.rsplit('-').next() {
            parts.push(regex::escape(variant));
        }
        for cpu in target.cpus() {
            parts.push(format!(r#"["']{}["']"#, regex::escape(cpu)));
        }
        let registers = target
            .registers_text()
            .lines()
            .filter(|line| !line.trim_start().starts_with('#'))
            .filter_map(|line| line.split_whitespace().next().map(str::to_string))
            .filter(|name| name.len() > 1)
            .map(|name| regex::escape(&name))
            .collect::<Vec<_>>();
        if !registers.is_empty() {
            let any = registers.join("|");
            parts.push(format!(r#"["'](?:{any})["']"#));
            parts.push(format!(r"\bRegister::(?:{any})\b"));
        }
        if let Some(bytes) = target.layout().segment_bytes {
            parts.push(number(bytes as u64));
            parts.push(number(bytes as u64 - 1));
        }
    }
    Regex::new(&format!("(?i)(?:{})", parts.join("|"))).expect("the facts pattern")
}

fn is_test_file(path: &Path) -> bool {
    // `path` is relative to the root: the root itself may sit under a `target/`.
    let name = path.file_name().and_then(|name| name.to_str()).unwrap_or("");
    let text = path.to_string_lossy();
    name == "tests.rs"
        || name.ends_with("_tests.rs")
        || name.starts_with("test_")
        || name.starts_with("testing")
        || name == "select_sweep.rs"
        || text.contains("tests/")
        || text.contains("/testing/")
        || text.starts_with("target/")
        || text.contains("/fixtures/")
        || text.contains("/.venv/")
}

fn files(root: &Path, directory: &Path, into: &mut Vec<PathBuf>) {
    let Ok(entries) = fs::read_dir(directory) else { return };
    for entry in entries.flatten() {
        let path = entry.path();
        if path.is_dir() {
            files(root, &path, into);
        } else if matches!(path.extension().and_then(|ext| ext.to_str()), Some("rs" | "py" | "sh")) && !is_test_file(path.strip_prefix(root).unwrap()) {
            into.push(path);
        }
    }
}

/// The lines of code of `path`: no comment lines, nothing from `#[cfg(test)]` on.
fn code(path: &Path) -> Vec<String> {
    let text = fs::read_to_string(path).unwrap_or_default();
    let comment = if path.extension().is_some_and(|ext| ext == "rs") { "//" } else { "#" };
    let mut lines = Vec::new();
    for line in text.lines() {
        if line.trim() == "#[cfg(test)]" {
            break;
        }
        if !line.trim_start().starts_with(comment) {
            lines.push(line.to_string());
        }
    }
    lines
}

fn counts(root: &Path) -> BTreeMap<String, usize> {
    let facts = facts();
    let mut all = Vec::new();
    for directory in SHARED {
        files(root, &root.join(directory), &mut all);
    }
    let mut found = BTreeMap::new();
    for path in all {
        let n = code(&path).iter().map(|line| facts.find_iter(line).count()).sum();
        if n > 0 {
            found.insert(path.strip_prefix(root).unwrap().to_string_lossy().into_owned(), n);
        }
    }
    found
}

fn baseline() -> BTreeMap<String, usize> {
    let text = fs::read_to_string(Path::new(ROOT).join(BASELINE)).unwrap_or_default();
    text.lines()
        .filter(|line| !line.trim().is_empty() && !line.starts_with('#'))
        .filter_map(|line| {
            let (count, path) = line.split_once(' ')?;
            Some((path.to_string(), count.parse().ok()?))
        })
        .collect()
}

/// What is wrong in the tree at `root` given the `allowed` copies per file.
fn problems(root: &Path, allowed: &BTreeMap<String, usize>) -> Vec<String> {
    let found = counts(root);
    let mut wrong = Vec::new();
    for (path, n) in &found {
        match allowed.get(path) {
            None => wrong.push(format!("{path}: {n} copies of a target fact, none allowed")),
            Some(a) if n > a => wrong.push(format!("{path}: {n} copies, was {a}")),
            Some(a) if n < a => wrong.push(format!("{path}: {n} copies, baseline says {a}: lower it (LLRM_BLESS=1)")),
            _ => {}
        }
    }
    for path in allowed.keys().filter(|path| !found.contains_key(*path)) {
        wrong.push(format!("{path}: none left, remove it from the baseline (LLRM_BLESS=1)"));
    }
    wrong
}

#[test]
fn shared_code_holds_no_copy_of_a_target_fact() {
    let root = Path::new(ROOT);
    if std::env::var_os("LLRM_BLESS").is_some() {
        let mut text = String::from("# Copies of target facts left in shared code, per file: count path. Only shrinks.\n");
        for (path, n) in &counts(root) {
            text += &format!("{n} {path}\n");
        }
        fs::write(root.join(BASELINE), text).unwrap();
        return;
    }
    let wrong = problems(root, &baseline());
    assert!(wrong.is_empty(), "read the target's description instead of copying it:\n{}", wrong.join("\n"));
}

/// A tree of one shared file holding `source`, under the build's scratch directory.
fn tree(name: &str, source: &str) -> PathBuf {
    let root = Path::new(env!("CARGO_TARGET_TMPDIR")).join("target_facts").join(name);
    let directory = root.join("crates/ir/sample/src");
    fs::create_dir_all(&directory).unwrap();
    fs::write(directory.join("lib.rs"), source).unwrap();
    root
}

const SAMPLE: &str = "crates/ir/sample/src/lib.rs";

/// The guard is an instrument: a register name added to a shared crate must
/// fail it, or a new copy would pass unseen.
#[test]
fn a_copy_added_to_shared_code_fails_the_guard() {
    let root = tree("added", "fn f() { let _ = \"ax\"; }\n");
    let wrong = problems(&root, &BTreeMap::new());
    assert_eq!(wrong, vec![format!("{SAMPLE}: 1 copies of a target fact, none allowed")]);
    let wrong = problems(&root, &BTreeMap::from([(SAMPLE.to_string(), 0)]));
    assert_eq!(wrong, vec![format!("{SAMPLE}: 1 copies, was 0")]);
}

/// A copy removed without lowering the baseline must fail too, or the baseline
/// goes stale and a later copy hides in the slack it left.
#[test]
fn a_copy_removed_without_shrinking_the_baseline_fails_the_guard() {
    let allowed = BTreeMap::from([(SAMPLE.to_string(), 2)]);
    let root = tree("removed", "fn f() { let _ = \"ax\"; }\n");
    assert_eq!(problems(&root, &allowed), vec![format!("{SAMPLE}: 1 copies, baseline says 2: lower it (LLRM_BLESS=1)")]);
    let root = tree("gone", "fn f() {}\n");
    assert_eq!(problems(&root, &allowed), vec![format!("{SAMPLE}: none left, remove it from the baseline (LLRM_BLESS=1)")]);
}

/// Copies in a comment or a test module are not copies in the code.
#[test]
fn comments_and_test_modules_are_not_counted() {
    let root = tree("quiet", "// the \"ax\" register\nfn f() {}\n#[cfg(test)]\nmod tests { fn g() { let _ = \"bx\"; } }\n");
    assert!(problems(&root, &BTreeMap::new()).is_empty());
}

/// Cost of the first version: a pattern that matched nothing passed for "no
/// copies". The facts the search is built from must be there to find.
#[test]
fn the_search_is_built_from_the_descriptions() {
    let facts = facts();
    for sample in ["\"x86-code16\"", "llrm_x86_code32::Code32", "\"486\"", "\"ax\"", "Register::BX", "0x10000", "0x1_0000", "65535", "0xFFFF"] {
        assert!(facts.is_match(sample), "{sample} is a target fact and is not found");
    }
    for sample in ["let x = 4;", "\"ordinary\"", "65537"] {
        assert!(!facts.is_match(sample), "{sample} is not a target fact");
    }
}
