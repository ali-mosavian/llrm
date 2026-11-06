//! No machine's paths in the crate: the examples took their tools from
//! `~/scratch/...` when the variable was unset, and ran on one machine only.

use std::path::Path;

fn sources(
    dir: &Path,
    out: &mut Vec<std::path::PathBuf>,
) {
    for entry in std::fs::read_dir(dir).unwrap() {
        let path = entry.unwrap().path();
        if path.is_dir() {
            sources(&path, out);
        } else if path.extension().is_some_and(|ext| ext == "rs") {
            out.push(path);
        }
    }
}

#[test]
fn no_source_names_a_path_on_one_machine() {
    let root = Path::new(env!("CARGO_MANIFEST_DIR"));
    let mut files = Vec::new();
    for dir in ["src", "examples", "tests"] {
        sources(&root.join(dir), &mut files);
    }
    let this = root.join("tests/hygiene.rs");
    let hits: Vec<_> = files
        .iter()
        .filter(|path| **path != this)
        .filter(|path| {
            let text = std::fs::read_to_string(path).unwrap();
            text.contains("/scratch") || text.contains("scratch/") || text.contains("/home/")
        })
        .collect();
    assert!(hits.is_empty(), "paths on one machine in {hits:?}");
}
