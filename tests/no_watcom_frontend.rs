//! The Watcom-dialect `llrm-c` front end is gone: `llrm-cc` over clang is the only C front end.

/// The workspace still had an `llrm-c` member, its bin and the `c-legacy`/`wccq`
/// features, 11.9 k lines nothing but opt-in tests built.
#[test]
fn the_workspace_has_no_llrm_c_member() {
    let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR"));
    assert!(!root.join("crates/frontends/llrm-c").exists(), "crates/frontends/llrm-c is back");
    assert!(!root.join("src/bin/llrm-c.rs").exists(), "src/bin/llrm-c.rs is back");
    let manifest = std::fs::read_to_string(root.join("Cargo.toml")).unwrap();
    for word in ["llrm-c =", "c-legacy", "wccq"] {
        assert!(!manifest.contains(word), "Cargo.toml mentions {word}");
    }
}
