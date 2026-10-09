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
use std::process::Command;

use regex::Regex;

const ROOT: &str = env!("CARGO_MANIFEST_DIR");
const BASELINE: &str = "tests/target_facts.baseline";
/// Shared code: everything but the target crates, the tests and the build
/// products.
const SHARED: [&str; 8] =
    ["crates/backend", "crates/ir", "crates/opt", "crates/support", "crates/bc", "crates/frontends", "src", "tools"];

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
        // The type (`M16`), what is named after the mode (`m16_options`,
        // `m16()`); not `-m16`, the flag.
        if let Some(variant) = name.rsplit('-').next() {
            let mut type_name = variant.to_owned();
            type_name[..1].make_ascii_uppercase();
            parts.push(format!(r"(?-i:\b{}\b)", regex::escape(&type_name)));
            parts.push(format!(r"(?-i:\b{}_)", regex::escape(variant)));
            parts.push(format!(r"(?-i:(?:^|[^-\w]){}\b)", regex::escape(variant)));
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
        if let Some(bytes) = target.layout().segment_bytes() {
            parts.push(number(bytes as u64));
            parts.push(number(bytes as u64 - 1));
        }
    }
    Regex::new(&format!("(?i)(?:{})", parts.join("|"))).expect("the facts pattern")
}

/// The one place that names the targets built in, which is its job
/// (docs/targets.md, the first principle).
const REGISTRY: &str = "crates/backend/llrm-driver/src/lib.rs";

fn is_test_file(path: &Path) -> bool {
    // `path` is relative to the root: the root itself may sit under a
    // `target/`.
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

/// The shared code git tracks: not a virtualenv, a build or an editor's
/// leavings beside it.
fn files(
    root: &Path,
    directory: &str,
    into: &mut Vec<PathBuf>,
) {
    let listed = std::process::Command::new("git")
        .arg("-C")
        .arg(root)
        .arg("ls-files")
        .arg("--")
        .arg(directory)
        .output()
        .expect("git runs");
    assert!(listed.status.success(), "git ls-files: {}", String::from_utf8_lossy(&listed.stderr));
    for line in String::from_utf8_lossy(&listed.stdout).lines() {
        let path = root.join(line);
        if matches!(
            path.extension().and_then(|ext| ext.to_str()),
            Some("rs" | "py" | "sh")
        )
            && !is_test_file(path.strip_prefix(root).unwrap())
            && line != REGISTRY
        {
            into.push(path);
        }
    }
}

/// The lines of code of `path`: no comment lines, and nothing of an item
/// `#[cfg(test)]` attributes.
fn code(path: &Path) -> Vec<String> {
    let text = fs::read_to_string(path).unwrap_or_default();
    let comment = if path.extension().is_some_and(|ext| ext == "rs") { "//" } else { "#" };
    code_of(&text, comment)
}

/// `code`, of the text. A `#[cfg(test)]` takes the item after it (a module, a
/// static, a function: to the end of its brackets or its `;`), not the rest of
/// the file: a test static once hid a target's spelling further down.
fn code_of(
    text: &str,
    comment: &str,
) -> Vec<String> {
    let mut lines = Vec::new();
    let mut skipping = false;
    let mut depth = 0i32;
    let mut started = false;
    for line in text.lines() {
        if skipping {
            // Attributes and doc lines between the cfg and the item are part of
            // it; so is everything to the end of the item.
            let code = line.split("//").next().unwrap_or("");
            for c in code.chars() {
                match c {
                    '(' | '[' | '{' => {
                        depth += 1;
                        started = true;
                    }
                    ')' | ']' | '}' => depth -= 1,
                    _ => {}
                }
            }
            let attribute = line.trim_start().starts_with("#[") || line.trim_start().starts_with("///");
            if !attribute && depth <= 0 && (started || code.trim_end().ends_with(';')) {
                skipping = false;
            }
            continue;
        }
        if line.trim() == "#[cfg(test)]" {
            skipping = true;
            depth = 0;
            started = false;
            continue;
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
        files(root, directory, &mut all);
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

/// The files the baseline text lists more than once: what a union merge of two
/// branches that lowered the same file's count leaves (`.gitattributes` merges
/// this file by union, so a merge never stops on it).
fn listed_twice(text: &str) -> Vec<String> {
    let mut seen = std::collections::BTreeSet::new();
    text.lines()
        .filter(|line| !line.trim().is_empty() && !line.starts_with('#'))
        .filter_map(|line| line.split_once(' ').map(|(_, path)| path.to_owned()))
        .filter(|path| !seen.insert(path.clone()))
        .collect()
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
fn problems(
    root: &Path,
    allowed: &BTreeMap<String, usize>,
) -> Vec<String> {
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
        let mut text =
            String::from("# Copies of target facts left in shared code, per file: count path. Only shrinks.\n");
        for (path, n) in &counts(root) {
            text += &format!("{n} {path}\n");
        }
        fs::write(root.join(BASELINE), text).unwrap();
        return;
    }
    let twice = listed_twice(&fs::read_to_string(root.join(BASELINE)).unwrap_or_default());
    assert!(
        twice.is_empty(),
        "the baseline lists {twice:?} twice, as a merge of two lowerings leaves it: LLRM_BLESS=1 cargo test --test target_facts"
    );
    let wrong = problems(root, &baseline());
    assert!(wrong.is_empty(), "read the target's description instead of copying it:\n{}", wrong.join("\n"));
}

/// A tree of one shared file holding `source`, under the build's scratch
/// directory.
fn tree(
    name: &str,
    source: &str,
) -> PathBuf {
    let root = Path::new(env!("CARGO_TARGET_TMPDIR")).join("target_facts").join(name);
    let _ = fs::remove_dir_all(&root);
    let directory = root.join("crates/ir/sample/src");
    fs::create_dir_all(&directory).unwrap();
    fs::write(directory.join("lib.rs"), source).unwrap();
    for args in [&["init", "-q"][..], &["add", "-A"][..]] {
        assert!(Command::new("git").arg("-C").arg(&root).args(args).output().unwrap().status.success());
    }
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
    assert_eq!(
        problems(&root, &allowed),
        vec![format!("{SAMPLE}: 1 copies, baseline says 2: lower it (LLRM_BLESS=1)")]
    );
    let root = tree("gone", "fn f() {}\n");
    assert_eq!(
        problems(&root, &allowed),
        vec![format!("{SAMPLE}: none left, remove it from the baseline (LLRM_BLESS=1)")]
    );
}

/// Copies in a comment or a test module are not copies in the code.
#[test]
fn comments_and_test_modules_are_not_counted() {
    let root =
        tree("quiet", "// the \"ax\" register\nfn f() {}\n#[cfg(test)]\nmod tests { fn g() { let _ = \"bx\"; } }\n");
    assert!(problems(&root, &BTreeMap::new()).is_empty());
}

/// Cost of the first version: a pattern that matched nothing passed for "no
/// copies". The facts the search is built from must be there to find.
#[test]
fn the_search_is_built_from_the_descriptions() {
    let facts = facts();
    for sample in [
        "\"x86-m16\"",
        "llrm_x86_m32::M32",
        "\"486\"",
        "\"ax\"",
        "Register::BX",
        "0x10000",
        "0x1_0000",
        "65535",
        "0xFFFF",
    ] {
        assert!(facts.is_match(sample), "{sample} is a target fact and is not found");
    }
    for sample in ["let x = 4;", "\"ordinary\"", "65537"] {
        assert!(!facts.is_match(sample), "{sample} is not a target fact");
    }
}

/// A virtualenv or a build beside the checkout held what the guard counts, so a
/// clean checkout and a working tree disagreed: only what git tracks is
/// counted.
#[test]
fn an_untracked_copy_is_not_counted() {
    let root = tree("untracked", "fn f() {}\n");
    let stray = root.join("crates/ir/sample/src/stray.rs");
    fs::write(&stray, "fn g() { let _ = \"ax\"; }\n").unwrap();
    assert!(problems(&root, &BTreeMap::new()).is_empty());
}

/// A merge kept both sides' lines for one file and the guard took the later: a
/// count nobody blessed. It is refused.
#[test]
fn a_file_listed_twice_is_found() {
    assert_eq!(listed_twice("# header\n3 a.rs\n2 b.rs\n1 a.rs\n"), ["a.rs"]);
    assert!(listed_twice("3 a.rs\n2 b.rs\n").is_empty());
}

/// `#[cfg(test)]` once ended the file's code: a test static in #1091 hid
/// copyprop's ESP spelling below it, and the ceiling was blessed down by
/// mistake. It takes the item it attributes, a module or a static or a
/// function, and no more.
#[test]
fn test_cfg_test_hides_its_item_and_not_the_rest_of_the_file() {
    let text = r#"fn a() { "shared"; }
#[cfg(test)]
static TABLE: [&str; 2] = [
    "in-test",
    "in-test",
];
fn b() { "after the static"; }
#[cfg(test)]
mod tests {
    fn c() { "in-test"; }
}
#[cfg(test)]
use std::x;
fn d() { "after the use"; }
"#;
    let kept = code_of(text, "//").join("\\n");
    assert!(kept.contains("after the static") && kept.contains("after the use") && kept.contains("shared"), "{kept}");
    assert!(!kept.contains("in-test"), "{kept}");
}
