//! The checked-in tables are what the pin's tblgen makes today.
//!
//! A table that no longer matches the generator, `features.toml` or the pin
//! fails here; `tools/dagisel/regen.sh` rewrites it. `DAGISEL_SKIP_PIN=1` skips
//! the test on a machine without the pin's source and build. The gate does not set it:
//! its `pintools` job builds the pin tree and exports `LLVM_SRC` and `LLVM_BUILD` (docs/working/ci.md).

use std::path::Path;
use std::process::Command;

#[test]
fn checked_in_tables_match_a_fresh_regeneration() {
    if std::env::var_os("DAGISEL_SKIP_PIN").is_some() {
        return;
    }
    let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../..");
    let checked_in = root.join("crates/backend/llrm-dagisel/src/generated");
    let fresh = tempfile::tempdir().unwrap();
    let status = Command::new(root.join("tools/dagisel/regen.sh"))
        .arg(fresh.path())
        .env("DAGISEL_GEN", env!("CARGO_BIN_EXE_llrm-dagisel-gen"))
        .status()
        .expect("run regen.sh");
    assert!(status.success(), "regen.sh failed; set DAGISEL_SKIP_PIN=1 where the pin is not installed");

    let mut names: Vec<_> = std::fs::read_dir(&checked_in).unwrap().map(|e| e.unwrap().file_name()).collect();
    names.sort();
    let mut fresh_names: Vec<_> = std::fs::read_dir(fresh.path()).unwrap().map(|e| e.unwrap().file_name()).collect();
    fresh_names.sort();
    assert_eq!(names, fresh_names, "the set of generated files changed");
    for name in names {
        let old = std::fs::read_to_string(checked_in.join(&name)).unwrap();
        let new = std::fs::read_to_string(fresh.path().join(&name)).unwrap();
        assert!(old == new, "{} is stale: run tools/dagisel/regen.sh", name.to_string_lossy());
    }
}
