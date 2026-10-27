//! Generated doc pages are current. Each page records the hash of its source
//! `.rs`; a rename or a reformat of a source leaves it stale, and the re-org
//! left 411 so.

use std::process::Command;

#[test]
fn test_no_generated_page_is_stale() {
    let root = env!("CARGO_MANIFEST_DIR");
    let out = Command::new("python3")
        .args(["tools/docs/docsbuild/build.py", "--check"])
        .current_dir(root)
        .output()
        .expect("python3 runs");
    let text = String::from_utf8_lossy(&out.stdout);
    let stale: Vec<&str> = text
        .lines()
        .filter(|line| {
            line.contains("generated docs are out of date")
                || line.contains("Contents list is stale")
                || line.starts_with("  docs/")
        })
        .collect();
    assert!(stale.is_empty(), "run python3 tools/docs/docsbuild/build.py --sync:\n{}", stale.join("\n"));
}
