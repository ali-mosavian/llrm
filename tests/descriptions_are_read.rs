//! A description nobody reads is a second definition nobody checks. Every description file of
//! every target is read by shared code (its name is in a source that reads it), and every key of
//! a TOML description is one a reader names.
//!
//! The files and keys are found in `crates/target/*`, not listed: a new description or a new key
//! with no reader fails here.

use std::collections::BTreeSet;
use std::fs;
use std::path::{Path, PathBuf};

const ROOT: &str = env!("CARGO_MANIFEST_DIR");

/// What a description names itself rather than the schema: a table keyed by a name or a number.
/// (file, path from the file's root, with `*` for any one name)
/// (`ports.*` and `foreign.*` are read through a closure that takes the key: `bound("low")`.)
const NAMED: [(&str, &str); 9] = [
    ("calling.toml", "*"),
    ("calling.toml", "*.symbol.*"),
    ("calling.toml", "*.result.*"),
    ("platform.toml", "physical.*"),
    ("datalayout.toml", "pointers.*"),
    ("pc-ports.toml", "port.*"),
    ("dos.toml", "port.*"),
    ("pc-ports.toml", "ports.*"),
    ("dos.toml", "foreign.*"),
];

fn descriptions() -> Vec<PathBuf> {
    let mut found = Vec::new();
    for entry in fs::read_dir(Path::new(ROOT).join("crates/target")).unwrap().flatten() {
        walk(&entry.path().join("src"), &mut found);
        for name in ["platform.toml"] {
            if entry.path().join(name).exists() {
                found.push(entry.path().join(name));
            }
        }
    }
    found.sort();
    found
}

fn walk(directory: &Path, found: &mut Vec<PathBuf>) {
    let Ok(entries) = fs::read_dir(directory) else { return };
    for entry in entries.flatten() {
        let path = entry.path();
        if path.is_dir() {
            walk(&path, found);
        } else if matches!(path.extension().and_then(|one| one.to_str()), Some("toml" | "regs" | "times" | "instr" | "isel" | "peep" | "legal")) || path.file_name().is_some_and(|one| one == "opcosts.txt") {
            found.push(path);
        }
    }
}

/// The source that reads descriptions: the target layer's non-test Rust (it parses them) and the tools' Python.
fn readers() -> String {
    let mut text = String::new();
    let mut stack = vec![Path::new(ROOT).join("crates/target"), Path::new(ROOT).join("tools")];
    while let Some(directory) = stack.pop() {
        let Ok(entries) = fs::read_dir(&directory) else { continue };
        for entry in entries.flatten() {
            let path = entry.path();
            let name = path.file_name().unwrap().to_string_lossy().into_owned();
            if path.is_dir() {
                if name != "tests" {
                    stack.push(path);
                }
            } else if name.ends_with(".py") && !name.starts_with("test_") {
                // The tools read the link recipe: `object.toml`'s `[link]`.
                text += &fs::read_to_string(&path).unwrap_or_default();
                text.push('\n');
            } else if name.ends_with(".rs") && !name.ends_with("_tests.rs") && name != "tests.rs" && !name.starts_with("test_") {
                let source = fs::read_to_string(&path).unwrap_or_default();
                // Not the tests of a module: they would read anything.
                text += source.split("\n#[cfg(test)]").next().unwrap_or("");
                text.push('\n');
            }
        }
    }
    text
}

fn keys(prefix: &str, value: &toml::Value, into: &mut Vec<(String, String)>) {
    match value {
        toml::Value::Table(table) => {
            for (key, one) in table {
                let path = if prefix.is_empty() { key.clone() } else { format!("{prefix}.{key}") };
                into.push((path.clone(), key.clone()));
                keys(&path, one, into);
            }
        }
        toml::Value::Array(items) => {
            for one in items {
                if one.is_table() {
                    keys(prefix, one, into);
                }
            }
        }
        _ => {}
    }
}

fn matches(pattern: &str, path: &str) -> bool {
    let (pattern, path): (Vec<&str>, Vec<&str>) = (pattern.split('.').collect(), path.split('.').collect());
    pattern.len() == path.len() && pattern.iter().zip(&path).all(|(one, other)| *one == "*" || one == other)
}

/// The files: each is named by a reader (`include_str!`, a `#[path]`, a build script).
#[test]
fn every_description_file_has_a_reader() {
    let mut sources = readers();
    for entry in fs::read_dir(Path::new(ROOT).join("crates")).unwrap().flatten() {
        for crate_dir in fs::read_dir(entry.path()).into_iter().flatten().flatten() {
            if let Ok(build) = fs::read_to_string(crate_dir.path().join("build.rs")) {
                sources += &build;
            }
        }
    }
    let unread: Vec<String> = descriptions()
        .into_iter()
        .filter(|path| path.file_name().is_some_and(|name| name != "Cargo.toml"))
        .filter(|path| !sources.contains(&*path.file_name().unwrap().to_string_lossy()))
        .map(|path| path.strip_prefix(ROOT).unwrap().display().to_string())
        .collect();
    assert!(unread.is_empty(), "no source names these descriptions; read them or delete them: {unread:?}");
}

/// The keys: each TOML key is one some reader names, bar the names a description gives its own tables.
#[test]
fn every_toml_key_has_a_reader() {
    let sources = readers();
    let mut unread = BTreeSet::new();
    for path in descriptions() {
        let name = path.file_name().unwrap().to_string_lossy().into_owned();
        if !name.ends_with(".toml") || name == "Cargo.toml" {
            continue;
        }
        let Ok(value) = fs::read_to_string(&path).unwrap().parse::<toml::Table>() else { continue };
        let mut all = Vec::new();
        keys("", &toml::Value::Table(value), &mut all);
        for (at, key) in all {
            let named = NAMED.iter().any(|(file, pattern)| *file == name && matches(pattern, &at));
            let read = [format!("get(\"{key}\")"), format!("[\"{key}\"]"), format!("remove(\"{key}\")"), format!("contains_key(\"{key}\")"), format!("\"{key}\" =>"), format!("\"{key}\","), format!("\"{key}\"]")];
            if !named && !read.iter().any(|one| sources.contains(one.as_str())) {
                unread.insert(format!("{}: {at}", path.strip_prefix(ROOT).unwrap().display()));
            }
        }
    }
    assert!(unread.is_empty(), "no reader names these keys; read them or delete them:\n{}", unread.into_iter().collect::<Vec<_>>().join("\n"));
}

/// The instrument: the walk finds the descriptions, and a key nobody names is reported.
#[test]
fn the_search_finds_descriptions_and_misses_a_key_nobody_reads() {
    let names: Vec<String> = descriptions().iter().map(|one| one.file_name().unwrap().to_string_lossy().into_owned()).collect();
    for expected in ["calling.toml", "datalayout.toml", "registers.regs", "timings.times", "x86.instr", "patterns.isel", "peephole.peep", "opcosts.txt", "object.toml"] {
        assert!(names.iter().any(|one| one == expected), "{expected} not found among {names:?}");
    }
    assert!(!readers().contains("\"a_key_no_reader_names_zz\""));
    assert!(matches("*.result.*", "cdecl32.result.1") && !matches("*.result.*", "cdecl32.result"));
}
