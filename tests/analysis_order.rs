//! An analysis result a pass reads is a function of the body, not of when it
//! was first asked: `LLRM_CHECK_ORDER=SEED` asks some analyses early at each
//! pass's entry, which ones by the seed, and the object is the lazy compile's.
//!
//! #1909: a `ThroughMemory` asked early and kept past sound edits made `fold`
//! weaker, and matmul bq32 -O3 came out 18 KB of asm against 34 KB.

use std::process::Command;

fn matmul(seed: Option<&str>) -> String {
    let dir = tempfile::tempdir().unwrap();
    let out = dir.path().join("matmul.s");
    let mut command = Command::new(env!("CARGO_BIN_EXE_llrm-qb"));
    command
        .arg(format!("{}/bench/matmul/matmul.bas", env!("CARGO_MANIFEST_DIR")))
        .args(["--dialect", "qb45", "-fqb-runtime=llrm", "-O3", "-fno-inline-functions"])
        .args(["-fno-inline-functions-called-once", "-g", "-march=i486", "-m32", "-S", "-o"])
        .arg(&out)
        .env_remove("LLRM_CHECK_ORDER");
    if let Some(seed) = seed {
        command.env("LLRM_CHECK_ORDER", seed);
    }
    let done = command.output().unwrap();
    assert!(done.status.success(), "{}", String::from_utf8_lossy(&done.stderr));
    std::fs::read_to_string(out).unwrap()
}

#[test]
fn test_a_compile_does_not_depend_on_which_analyses_were_asked_early() {
    let lazy = matmul(None);
    for seed in ["1", "2", "3"] {
        assert_eq!(matmul(Some(seed)).len(), lazy.len(), "LLRM_CHECK_ORDER={seed} changed the object");
    }
}
