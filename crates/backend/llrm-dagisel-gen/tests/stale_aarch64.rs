//! The checked-in AArch64 table is what the pin's tblgen makes today.
//!
//! A table that no longer matches the generator, `features_aarch64.toml` or the pin fails here;
//! `aarch64/regen.sh` rewrites it. `DAGISEL_SKIP_PIN=1` skips the test on a machine without the
//! pin's source and build.

use std::path::Path;
use std::process::Command;

#[test]
fn the_checked_in_aarch64_table_matches_a_fresh_regeneration() {
    if std::env::var_os("DAGISEL_SKIP_PIN").is_some() {
        return;
    }
    let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../..");
    let checked_in = root.join("crates/backend/llrm-dagisel/src/generated/aarch64");
    let fresh = tempfile::tempdir().unwrap();
    let status = Command::new(Path::new(env!("CARGO_MANIFEST_DIR")).join("aarch64/regen.sh"))
        .arg(fresh.path())
        .env("DAGISEL_GEN", env!("CARGO_BIN_EXE_llrm-dagisel-gen"))
        .status()
        .expect("run regen.sh");
    assert!(status.success(), "aarch64/regen.sh failed; set DAGISEL_SKIP_PIN=1 where the pin is not installed");

    let fresh = fresh.path().join("aarch64");
    let list = |dir: &Path| {
        let mut names: Vec<_> = std::fs::read_dir(dir).unwrap().map(|e| e.unwrap().file_name()).collect();
        names.sort();
        names
    };
    assert_eq!(list(&checked_in), list(&fresh), "the set of generated files changed");
    for name in list(&checked_in) {
        let old = std::fs::read_to_string(checked_in.join(&name)).unwrap();
        let new = std::fs::read_to_string(fresh.join(&name)).unwrap();
        assert!(old == new, "{} is stale: run aarch64/regen.sh", name.to_string_lossy());
    }
}
