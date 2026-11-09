//! A frame whose first push is one register keeps the pre-index range of a
//! single store (-256), not the pair's (-512).

use std::process::Command;

/// `store_merge_interleaved.ll` saved one register at O1 with `str x30, [sp, #-272]!`, which
/// does not encode: the object failed with "a pre-index offset -272 does not fit 9 bits".
#[test]
fn a_single_register_first_push_does_not_take_the_pairs_range() {
    for level in ["-O1", "-O2", "-Os"] {
        let out = std::env::temp_dir().join("llrm-single-push-bump.o");
        let _ = std::fs::remove_file(&out);
        let done = Command::new(env!("CARGO_BIN_EXE_llrm-arm64-llc"))
            .args([level, "-o", out.to_str().unwrap(), concat!(env!("CARGO_MANIFEST_DIR"), "/tests/data/store_merge_interleaved.ll")])
            .output()
            .expect("the driver runs");
        assert!(done.status.success(), "{level}: {}", String::from_utf8_lossy(&done.stderr));
        assert!(out.exists(), "{level}: no object");
    }
}
