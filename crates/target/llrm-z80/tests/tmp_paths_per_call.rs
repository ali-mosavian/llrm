//! A test that wrote into a fixed `/tmp/llrm-...` path raced with the same test
//! in another session (m68k `llc_flags::excess_precision` failed on 2026-11-09
//! on another run's `f.ll`). Scratch space is a `tempfile::tempdir()`, unique
//! per call and removed on drop.

#[test]
fn scratch_paths_in_tests_are_not_in_the_shared_temp_dir() {
    let shared = ["std::env::", "temp_dir()"].concat();
    let this = concat!(env!("CARGO_MANIFEST_DIR"), "/tests/tmp_paths_per_call.rs");
    let mut bad = Vec::new();
    for entry in std::fs::read_dir(concat!(env!("CARGO_MANIFEST_DIR"), "/tests")).unwrap() {
        let path = entry.unwrap().path();
        if path.extension().is_some_and(|e| e == "rs")
            && path.to_str() != Some(this)
            && std::fs::read_to_string(&path).unwrap().contains(&shared)
        {
            bad.push(path.file_name().unwrap().to_string_lossy().into_owned());
        }
    }
    assert!(bad.is_empty(), "shared temp dir used in {bad:?}");
}
