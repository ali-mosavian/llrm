//! A test that wrote into a fixed `/tmp/llrm-...` directory raced with the same
//! test in another session (m68k `llc_flags::excess_precision` failed on
//! 2026-11-09 with another run's half-written `f.ll`). A scratch directory
//! carries the process id.

#[test]
fn scratch_paths_in_tests_are_not_fixed() {
    let fixed = ["temp_dir()", ".join(\""].concat();
    let mut bad = Vec::new();
    for entry in std::fs::read_dir(concat!(env!("CARGO_MANIFEST_DIR"), "/tests")).unwrap() {
        let path = entry.unwrap().path();
        if path.extension().is_some_and(|e| e == "rs")
            && path.file_name().is_some_and(|n| n != "tmp_paths_per_process.rs")
        {
            let source = std::fs::read_to_string(&path).unwrap();
            if source.lines().any(|line| line.contains(&fixed)) {
                bad.push(path.file_name().unwrap().to_string_lossy().into_owned());
            }
        }
    }
    assert!(bad.is_empty(), "fixed scratch directory in {bad:?}");
}
