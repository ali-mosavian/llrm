//! LiveRangeShrink past a call frame at -m32.

use std::process::Command;

/// Since a call-frame pair surrounds every call, a block's phis were taken to
/// start every stretch of the block: the sum of two phis moved above the calls,
/// and the sum of that and a third phi moved above it. The compile died in the
/// live-range calculation, "a unique value" (zerofill_spills.bas at -m32 -O1,
/// -O2 and -Os).
#[test]
fn a_sum_of_phis_after_calls_compiles_at_m32() {
    let source = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/data/zerofill_sum_after_calls.bas");
    let directory = tempfile::TempDir::new().unwrap();
    for level in ["-O1", "-O2", "-Os"] {
        let run = Command::new(env!("CARGO_BIN_EXE_llrm-qb"))
            .arg(&source)
            .args(["-m32", level, "-o"])
            .arg(directory.path().join("out.obj"))
            .output()
            .unwrap();
        assert!(run.status.success(), "{level}: {}", String::from_utf8_lossy(&run.stderr));
    }
}
