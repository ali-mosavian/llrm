//! The crate graph of a target (docs/targets.md, row 19b): a data crate holds
//! the description and does not depend on `llrm-core`; its select crate holds
//! what is generated from it and does; only `llrm-driver` depends on a select
//! crate, so a product names a target in one place.

use std::path::Path;

/// The names under `[section]` of a manifest (the dependency tables).
fn names(
    manifest: &str,
    section: &str,
) -> Vec<String> {
    let mut found = Vec::new();
    let mut inside = false;
    for line in manifest.lines() {
        let line = line.trim();
        if line.starts_with('[') {
            inside = line == format!("[{section}]");
        } else if inside && !line.is_empty() && !line.starts_with('#') {
            if let Some((name, _)) = line.split_once('=') {
                found.push(name.trim().to_owned());
            }
        }
    }
    found
}

fn manifests(root: &Path) -> Vec<(String, String)> {
    let mut found = Vec::new();
    for group in std::fs::read_dir(root.join("crates")).unwrap().flatten() {
        for crate_dir in std::fs::read_dir(group.path()).unwrap().flatten() {
            let path = crate_dir.path().join("Cargo.toml");
            if let Ok(text) = std::fs::read_to_string(&path) {
                found.push((crate_dir.file_name().to_string_lossy().into_owned(), text));
            }
        }
    }
    found
}

#[test]
fn data_crates_do_not_depend_on_core_and_select_crates_do() {
    let all = manifests(Path::new(env!("CARGO_MANIFEST_DIR")));
    let selects: Vec<_> =
        all.iter().filter(|(name, _)| name.starts_with("llrm-x86-m") && name.ends_with("-select")).collect();
    let data: Vec<_> =
        all.iter().filter(|(name, _)| name.starts_with("llrm-x86-m") && !name.ends_with("-select")).collect();
    assert!(!selects.is_empty() && !data.is_empty(), "no target crates found");
    for (name, text) in &data {
        for section in ["dependencies", "dev-dependencies"] {
            assert!(!names(text, section).iter().any(|one| one == "llrm-core"), "{name} [{section}] names llrm-core");
        }
    }
    for (name, text) in &selects {
        assert!(
            names(text, "dependencies").iter().any(|one| one == "llrm-core"),
            "{name} does not depend on llrm-core"
        );
    }
}

#[test]
fn only_the_driver_depends_on_a_select_crate() {
    let all = manifests(Path::new(env!("CARGO_MANIFEST_DIR")));
    let users: Vec<&str> = all
        .iter()
        .filter(|(name, _)| name != "llrm-driver")
        .filter(|(_, text)| {
            ["dependencies", "dev-dependencies", "build-dependencies"].iter().any(|section| {
                names(text, section).iter().any(|one| one.starts_with("llrm-x86-m") && one.ends_with("-select"))
            })
        })
        .map(|(name, _)| name.as_str())
        .collect();
    assert!(users.is_empty(), "{users:?} depend on a select crate");
}
