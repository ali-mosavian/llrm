//! A Microsoft runtime on a 32-bit target is refused by name.

use std::process::Command;

/// `PRINT "hi"` with `--runtime qb45 -m32` died with "@$string3: overlapping
/// or pre-added relocations", which names neither flag. The qb45 runtime is
/// 16-bit code; the 32-bit one is `-fqb-runtime=llrm`.
#[test]
fn a_microsoft_runtime_on_a_32_bit_target_says_to_use_the_llrm_runtime() {
    let scratch = tempfile::tempdir().unwrap();
    std::fs::write(scratch.path().join("a.bas"), "PRINT \"hi\"\n").unwrap();
    let output = Command::new(env!("CARGO_BIN_EXE_llrm-qb"))
        .current_dir(scratch.path())
        .args(["a.bas", "--dialect", "qb45", "--runtime", "qb45", "-m32", "-o", "a.obj"])
        .output()
        .unwrap();
    let said = String::from_utf8_lossy(&output.stderr);
    assert!(!output.status.success());
    assert!(said.contains("-fqb-runtime=llrm"), "{said}");
}
