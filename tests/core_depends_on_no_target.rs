//! `llrm-core` is the shared backend: its normal dependencies name no target
//! crate. A target is a dev-dependency there until the inversion (row 19b) is
//! done, so a production use of one fails to build.

#[test]
fn llrm_core_has_no_target_crate_among_its_dependencies() {
    let manifest = include_str!("../crates/backend/llrm-core/Cargo.toml");
    let dependencies =
        manifest.split("\n[").find(|section| section.starts_with("dependencies]")).expect("a [dependencies] section");
    let targets: Vec<&str> = dependencies
        .lines()
        .filter_map(|line| line.split('=').next().map(str::trim))
        .filter(|name| name.starts_with("llrm-x86-m"))
        .collect();
    assert!(targets.is_empty(), "llrm-core depends on {targets:?}");
}
