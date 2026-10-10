//! llrm-core generates a selector, peephole rules and effect rows for its own
//! tests behind its `fixtures` feature. A product binds a target's from the
//! target's select crate: no other crate may switch the feature on, or the
//! product would carry a second copy.

use std::path::Path;

fn manifests(
    dir: &Path,
    found: &mut Vec<std::path::PathBuf>,
) {
    for entry in std::fs::read_dir(dir).unwrap().flatten() {
        let path = entry.path();
        let name = entry.file_name();
        if path.is_dir() && name != "target" && name != ".git" && name != "node_modules" {
            manifests(&path, found);
        } else if name == "Cargo.toml" {
            found.push(path);
        }
    }
}

#[test]
fn only_llrm_core_turns_on_its_fixtures() {
    let root = Path::new(env!("CARGO_MANIFEST_DIR"));
    let mut found = Vec::new();
    manifests(&root.join("crates"), &mut found);
    manifests(&root.join("src"), &mut found);
    found.push(root.join("Cargo.toml"));
    let core = root.join("crates/backend/llrm-core/Cargo.toml");
    let users: Vec<_> = found
        .iter()
        .filter(|one| **one != core)
        .filter(|one| std::fs::read_to_string(one).unwrap().contains("fixtures"))
        .collect();
    assert!(users.is_empty(), "{users:?} name the fixtures feature");
}
