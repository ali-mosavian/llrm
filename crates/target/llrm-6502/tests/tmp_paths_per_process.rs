//! A test that wrote into a fixed temp directory raced with the same test in
//! another session (m68k `llc_flags::excess_precision`, 2026-11-09: another
//! run's half-written `f.ll`). Scratch directories are `tempfile::tempdir()`s
//! or carry the process id and a call counter.

#[test]
fn scratch_paths_in_tests_are_not_fixed() {
    let fixed = ["temp_dir()", ".join(\""].concat();
    let mut bad = Vec::new();
    for entry in std::fs::read_dir(concat!(env!("CARGO_MANIFEST_DIR"), "/tests")).unwrap() {
        let path = entry.unwrap().path();
        if path.extension().is_some_and(|e| e == "rs") && std::fs::read_to_string(&path).unwrap().contains(&fixed) {
            bad.push(path.file_name().unwrap().to_string_lossy().into_owned());
        }
    }
    assert!(bad.is_empty(), "fixed scratch directory in {bad:?}");
}
