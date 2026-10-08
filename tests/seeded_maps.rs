//! std's `HashMap`/`HashSet` are seeded per run: a pass's work and any order that reaches output change from one compile to
//! the next (#992: `callees` moved a pass's count 1% and flaked the scaling gate). Every use names `llrm_support::hash`'s.
//! The same rule is `clippy.toml`'s `disallowed-types`; this one needs no clippy run.

use std::fs;
use std::path::Path;

const ROOT: &str = env!("CARGO_MANIFEST_DIR");
/// Where the aliases are defined: llrm-mir depends on nothing, so it has its own.
const ALIASES: [&str; 2] = ["crates/support/llrm-support/src/hash.rs", "crates/ir/llrm-mir/src/hash.rs"];

fn sources(dir: &Path, out: &mut Vec<String>) {
    for entry in fs::read_dir(dir).unwrap().flatten() {
        let path = entry.path();
        if path.is_dir() {
            if path.file_name().is_some_and(|name| name != "target") {
                sources(&path, out);
            }
        } else if path.extension().is_some_and(|ext| ext == "rs") {
            out.push(path.strip_prefix(ROOT).unwrap().to_string_lossy().into_owned());
        }
    }
}

#[test]
fn test_no_source_names_a_randomly_seeded_std_map() {
    let mut files = Vec::new();
    for dir in ["crates", "src", "tests"] {
        if Path::new(ROOT).join(dir).is_dir() {
            sources(&Path::new(ROOT).join(dir), &mut files);
        }
    }
    let mut found = Vec::new();
    for file in files.iter().filter(|file| !ALIASES.contains(&file.as_str()) && file.as_str() != "tests/seeded_maps.rs") {
        for (number, line) in fs::read_to_string(Path::new(ROOT).join(file)).unwrap().lines().enumerate() {
            let code = line.split("//").next().unwrap();
            let std_set = code.contains("collections::HashSet") || code.contains("collections::HashMap") || code.contains("RandomState");
            let grouped = code.contains("std::collections::{") && (code.contains("HashMap") || code.contains("HashSet"));
            if std_set || grouped {
                found.push(format!("{file}:{}: {}", number + 1, line.trim()));
            }
        }
    }
    assert!(found.is_empty(), "use llrm_support::hash::{{HashMap, HashSet}}:\n{}", found.join("\n"));
}
