//! An object is one number whatever facts a result about it was made with.

mod common;

/// `i64_param_home.c` at -O2 -m32: a frame slot whose address an edit stopped
/// exposing was object 15 in the result kept past the edit and 17 in one
/// derived after it, so a query between them found two objects that cannot
/// alias and the home store of the i64 parameter went dead (68 for 78).
/// `LLRM_CHECK_REPLAY` has every kept result agree with deriving it afresh.
#[test]
fn test_a_kept_result_and_a_fresh_one_name_a_frame_slot_alike() {
    let source = format!("{}/tests/run/c/i64_param_home.c", env!("CARGO_MANIFEST_DIR"));
    let scratch = tempfile::tempdir().unwrap();
    let output = common::cc::llrm_c()
        .current_dir(scratch.path())
        .env("LLRM_CHECK_REPLAY", "1")
        .args(["-O2", "-march=i486", "-m32", "-S", "-o", "a.s", &source])
        .output()
        .unwrap();
    assert!(output.status.success(), "{}", String::from_utf8_lossy(&output.stderr));
}
