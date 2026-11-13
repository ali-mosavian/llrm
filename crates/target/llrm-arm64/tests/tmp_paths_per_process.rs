//! A test that wrote into a fixed `/tmp/llrm-...` path raced with the same test
//! in another session (llrm-m68k `llc_flags` failed 2026-11-09 on another run's
//! half-written file). Scratch paths are `tempfile::tempdir()`s: unique per
//! call, removed on drop.

#[test]
fn scratch_paths_in_tests_are_not_fixed() {
    let fixed = "temp_dir()";
    let mut bad = Vec::new();
    for entry in std::fs::read_dir(concat!(env!("CARGO_MANIFEST_DIR"), "/tests")).unwrap() {
        let path = entry.unwrap().path();
        if path.extension().is_some_and(|e| e == "rs")
            && path.file_name().is_some_and(|n| n != "tmp_paths_per_process.rs")
            && std::fs::read_to_string(&path).unwrap().contains(fixed)
        {
            bad.push(path.file_name().unwrap().to_string_lossy().into_owned());
        }
    }
    assert!(bad.is_empty(), "fixed scratch path in {bad:?}");
}
