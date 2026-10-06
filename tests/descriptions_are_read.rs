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
const NAMED: &[(&str, &str)] = &[
    ("calling.toml", "*"),
    ("calling.toml", "*.symbol.*"),
    ("calling.toml", "*.result.*"),
    ("platform.toml", "physical.*"),
    ("datalayout.toml", "pointers.*"),
    ("pc-ports.toml", "port.*"),
    ("dos.toml", "port.*"),
    ("pc-ports.toml", "ports.*"),
    ("dos.toml", "foreign.*"),
    // Every integer of an OS facts file is defined for the assembler as -DDOS_<KEY>: the mechanism reads them all.
    ("facts.toml", "*"),
    ("facts.toml", "errors.*"),
    ("os.toml", "heap_bytes"),
    // The error codes are a table the interface maps over, whatever their names.
    ("interface.toml", "errors.*"),
];

/// The files git tracks under `directories` of `root`: what the repository holds, not what a build,
/// a virtualenv or an editor left beside it (a `tools/.venv` held every key's name and let a key
/// nobody reads pass).
fn tracked(root: &Path, directories: &[&str]) -> Vec<PathBuf> {
    let listed = std::process::Command::new("git").arg("-C").arg(root).arg("ls-files").arg("--").args(directories).output().expect("git runs");
    assert!(listed.status.success(), "git ls-files: {}", String::from_utf8_lossy(&listed.stderr));
    String::from_utf8_lossy(&listed.stdout).lines().map(|line| root.join(line)).collect()
}

fn descriptions() -> Vec<PathBuf> {
    let mut found: Vec<PathBuf> = tracked(Path::new(ROOT), &["crates/target", "runtime"])
        .into_iter()
        .filter(|path| {
            let name = path.file_name().unwrap().to_string_lossy();
            let in_src = path.components().any(|one| one.as_os_str() == "src" || one.as_os_str() == "runtime") || name == "platform.toml";
            in_src && (matches!(path.extension().and_then(|one| one.to_str()), Some("toml" | "regs" | "times" | "instr" | "isel" | "peep" | "legal")) || name == "opcosts.txt")
        })
        .collect();
    found.sort();
    found
}

/// The source that reads descriptions: the non-test Rust of the crates (the target layer parses them, the
/// frontends read their runtime's) and the tools' Python.
fn readers() -> String {
    readers_in(Path::new(ROOT))
}

fn readers_in(root: &Path) -> String {
    let mut text = String::new();
    for path in tracked(root, &["crates", "tools"]) {
        let name = path.file_name().unwrap().to_string_lossy().into_owned();
        let relative = path.strip_prefix(root).unwrap().to_string_lossy().into_owned();
        if relative.contains("/tests/") {
            continue;
        }
        if name.ends_with(".py") && !name.starts_with("test_") {
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
    for path in tracked(Path::new(ROOT), &["crates"]) {
        if path.file_name().is_some_and(|name| name == "build.rs") {
            sources += &fs::read_to_string(&path).unwrap_or_default();
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
        let text = fs::read_to_string(&path).unwrap();
        let Ok(value) = text.parse::<toml::Table>() else { continue };
        let mut all = Vec::new();
        keys("", &toml::Value::Table(value), &mut all);
        for (at, key) in all {
            let named = NAMED.iter().any(|(file, pattern)| *file == name && matches(pattern, &at));
            // A reader that takes the key through a closure or a helper: `name("code")`, `number("fixed")`.
            let called = regex::Regex::new(&format!(r#"(?:\w\(|,\s*)"{}"\s*[,)]"#, regex::escape(&key))).unwrap();
            // `assembler_defines = ["stack_base:STACK_BYTES"]`: a key the description itself names for the assembler.
            let defined = text.contains(&format!("\"{key}:"));
            let read = [format!("get(\"{key}\")"), format!("[\"{key}\"]"), format!("remove(\"{key}\")"), format!("contains_key(\"{key}\")"), format!("\"{key}\" =>"), format!("\"{key}\","), format!("\"{key}\"]")];
            if !named && !defined && !called.is_match(&sources) && !read.iter().any(|one| sources.contains(one.as_str())) {
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

/// The walk read every file beside the checkout, and a virtualenv under `tools` holding a key's name
/// let three keys nobody reads pass on one machine and fail on a clean one: only what git tracks counts.
#[test]
fn an_untracked_file_is_not_a_reader() {
    let root = Path::new(env!("CARGO_TARGET_TMPDIR")).join("descriptions_tracked");
    let _ = fs::remove_dir_all(&root);
    fs::create_dir_all(root.join("tools/.venv/lib")).unwrap();
    fs::create_dir_all(root.join("crates/target/llrm-x/src")).unwrap();
    let git = |args: &[&str]| assert!(std::process::Command::new("git").arg("-C").arg(&root).args(args).output().unwrap().status.success());
    git(&["init", "-q"]);
    fs::write(root.join("tools/reads.py"), "x = table[\"tracked_key\"]\n").unwrap();
    fs::write(root.join("tools/.venv/lib/site.py"), "y = \"unread_key\"\n").unwrap();
    git(&["add", "tools/reads.py"]);
    let found = readers_in(&root);
    assert!(found.contains("tracked_key") && !found.contains("unread_key"), "{found}");
}
