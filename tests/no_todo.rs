//! `todo!()` and `unimplemented!()` in non-test code are a compile that panics
//! on input nobody tried: the SCEV expander's `isSafeToExpand` was a `todo!`
//! and a loop's value expanded after the loop crashed `llrm-c`. No crate code
//! may hold one; the baseline is empty and may only stay so.

use std::fs;
use std::path::Path;

const ROOT: &str = env!("CARGO_MANIFEST_DIR");

fn sources(
    dir: &Path,
    out: &mut Vec<String>,
) {
    for entry in fs::read_dir(dir).unwrap().flatten() {
        let path = entry.path();
        if path.is_dir() {
            if path.file_name().is_some_and(|name| name != "target" && name != "tests") {
                sources(&path, out);
            }
        } else if path.extension().is_some_and(|ext| ext == "rs") {
            let name = path.file_name().unwrap().to_string_lossy();
            if name != "tests.rs" && !name.ends_with("_tests.rs") {
                out.push(path.strip_prefix(ROOT).unwrap().to_string_lossy().into_owned());
            }
        }
    }
}

/// The lines (1-based) of `text` that call `todo!` or `unimplemented!` in code:
/// not a comment, not after the first `#[cfg(test)]` (the tests module that
/// closes a file).
fn found(text: &str) -> Vec<usize> {
    let mut hits = Vec::new();
    for (number, line) in text.lines().enumerate() {
        if line.trim_start().starts_with("#[cfg(test)]") {
            break;
        }
        let code = line.split("//").next().unwrap();
        if code.contains("todo!") || code.contains("unimplemented!") {
            hits.push(number + 1);
        }
    }
    hits
}

#[test]
fn test_a_planted_todo_is_found_and_a_comment_or_a_test_module_is_not() {
    assert_eq!(found("fn a() {\n    todo!()\n}\n"), vec![2]);
    assert_eq!(found("fn a() { unimplemented!(\"x\") }\n"), vec![1]);
    assert!(found("// todo!() later\nfn a() {}\n").is_empty());
    assert!(found("fn a() {}\n#[cfg(test)]\nmod t { fn b() { todo!() } }\n").is_empty());
}

#[test]
fn test_no_crate_code_holds_a_todo_or_unimplemented() {
    let mut files = Vec::new();
    for dir in ["crates", "src"] {
        if Path::new(ROOT).join(dir).is_dir() {
            sources(&Path::new(ROOT).join(dir), &mut files);
        }
    }
    let mut hits = Vec::new();
    for file in &files {
        for number in found(&fs::read_to_string(Path::new(ROOT).join(file)).unwrap_or_default()) {
            hits.push(format!("{file}:{number}"));
        }
    }
    assert!(
        hits.is_empty(),
        "todo!/unimplemented! in code that ships; make it an error the caller can report:\n{}",
        hits.join("\n")
    );
}
