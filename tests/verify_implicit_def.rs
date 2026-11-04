//! `LLRM_VERIFY` read a register no definition reaches: the allocator spilled
//! an undefined value (`implicit_def`) with a store, and `parcopy` dropped the
//! definition, so examples/desk.nib failed "Desk.claim reads eax before every
//! path writes it" at -O1 and above (tests/run, and the landing runner's
//! LLRM_CHECK_STALE). The pin remakes an `IMPLICIT_DEF` where it is read.

use std::process::Command;

#[test]
fn a_spilled_undefined_value_is_not_stored_from_a_register_nothing_wrote() {
    let scratch = tempfile::tempdir().unwrap();
    let root = env!("CARGO_MANIFEST_DIR");
    let object = scratch.path().join("desk.obj");
    for level in ["-O1", "-O2"] {
        let done = Command::new(env!("CARGO_BIN_EXE_llrm-nib"))
            .args(["-m32", level, "-o"])
            .arg(&object)
            .arg(format!("{root}/examples/desk.nib"))
            .env("LLRM_VERIFY", "1")
            .output()
            .unwrap();
        assert!(done.status.success(), "{level}: {}", String::from_utf8_lossy(&done.stderr));
    }
}
