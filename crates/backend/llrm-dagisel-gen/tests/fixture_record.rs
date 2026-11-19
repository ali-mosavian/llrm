//! The per-target generator on a record of its own: the pin's M68k.td to
//! matcher tables, twice, identical.
//!
//! No target crate here has a record yet, so `stale_targets.rs` has nothing to
//! regenerate; this keeps `regen-target.sh` and `--target` exercised.
//! `DAGISEL_SKIP_PIN=1` skips the test where the pin is not installed.

use std::path::Path;
use std::process::Command;

fn regenerated(into: &Path) -> Vec<(String, String)> {
    let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../..");
    let status = Command::new(root.join("tools/dagisel/regen-target.sh"))
        .arg(Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/m68k/dagisel.toml"))
        .arg(into)
        .env("DAGISEL_GEN", env!("CARGO_BIN_EXE_llrm-dagisel-gen"))
        .status()
        .expect("run regen-target.sh");
    assert!(status.success(), "regen-target.sh failed on the fixture record; DAGISEL_SKIP_PIN=1 skips");
    let mut files: Vec<_> = std::fs::read_dir(into)
        .unwrap()
        .map(|e| {
            let path = e.unwrap().path();
            (path.file_name().unwrap().to_string_lossy().into_owned(), std::fs::read_to_string(path).unwrap())
        })
        .collect();
    files.sort();
    files
}

#[test]
fn the_fixture_record_generates_the_same_tables_twice() {
    if std::env::var_os("DAGISEL_SKIP_PIN").is_some() {
        return;
    }
    let (a, b) = (tempfile::tempdir().unwrap(), tempfile::tempdir().unwrap());
    let (first, second) = (regenerated(a.path()), regenerated(b.path()));
    let names: Vec<_> = first.iter().map(|(name, _)| name.as_str()).collect();
    assert!(names.contains(&"ids.rs") && names.contains(&"m68k_m68000.rs"), "unexpected files: {names:?}");
    assert!(first == second, "two runs of the generator differ");
}
