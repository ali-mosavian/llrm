//! The checked-in tables compile under `-D warnings` (CI's setting): a table with no open pattern predicate left `ctx`
//! unused in `pattern_predicate`, and the first build with warnings denied failed (`aarch64_apple_m1.rs`).

use std::path::{Path, PathBuf};

fn tables(dir: &Path, out: &mut Vec<PathBuf>) {
    for entry in std::fs::read_dir(dir).unwrap() {
        let path = entry.unwrap().path();
        if path.is_dir() {
            tables(&path, out);
        } else if path.extension().is_some_and(|ext| ext == "rs") {
            out.push(path);
        }
    }
}

#[test]
fn a_pattern_predicate_with_no_arm_does_not_bind_ctx() {
    let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../..");
    let mut files = Vec::new();
    tables(&root.join("crates/backend/llrm-dagisel/src/generated"), &mut files);
    let mut seen = 0;
    let mut bad = Vec::new();
    for file in files {
        let text = std::fs::read_to_string(&file).unwrap();
        let Some(at) = text.find("pub fn pattern_predicate<") else { continue };
        seen += 1;
        let body = &text[at..];
        let body = &body[..body.find("\n}\n").expect("the function ends")];
        let arms = body.lines().filter(|line| line.trim_start().starts_with(|c: char| c.is_ascii_digit())).count();
        let named = body.lines().next().unwrap().contains("(ctx:");
        if arms == 0 && named {
            bad.push(file.display().to_string());
        }
    }
    assert!(seen > 0, "no generated table was found");
    assert!(bad.is_empty(), "ctx is unused in {bad:?}");
}
