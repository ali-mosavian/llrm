//! A test that wrote into a scratch dir with a fixed name raced with the same
//! test in another session (tests/refuse_vector.rs shared three such dirs;
//! llrm-m68k `llc_flags` failed 2026-11-09 on another run's half-written file).
//! Scratch paths are `tempfile::tempdir()`s: unique per call, removed on drop.

#[test]
fn scratch_paths_in_tests_are_not_a_fixed_name() {
    let fixed = concat!("temp_dir()", ".join(\"");
    let mut bad = Vec::new();
    for entry in std::fs::read_dir(concat!(env!("CARGO_MANIFEST_DIR"), "/tests")).unwrap() {
        let path = entry.unwrap().path();
        if path.extension().is_some_and(|e| e == "rs") && std::fs::read_to_string(&path).unwrap().contains(fixed) {
            bad.push(path.file_name().unwrap().to_string_lossy().into_owned());
        }
    }
    assert!(bad.is_empty(), "fixed scratch path in {bad:?}");
}
