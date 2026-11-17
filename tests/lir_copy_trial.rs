//! At m32 -Os the object decides between a copy loop and the `memcpy` call that
//! replaces it.

mod common;

/// x_radix at -Os turned `a[i] = b[i]` over n words into `call memcpy` (3
/// pushes, call, `add esp`, a spill of the byte count against a 19-byte loop)
/// and was 294 B against gcc's 287. The trial compile that keeps the loop is
/// smaller (284) and stays.
#[test]
fn at_os_a_copy_loop_that_makes_the_object_smaller_stays_a_loop() {
    let source = format!("{}/crates/target/llrm-x86-m32/vsgcc/kernels/x_radix/x_radix.c", env!("CARGO_MANIFEST_DIR"));
    let scratch = tempfile::tempdir().unwrap();
    let output = common::cc::llrm_c()
        .current_dir(scratch.path())
        .args(["-Os", "-m32", "-mabi=sysv", "-march=i486", "-S", "-o", "a.s", &source])
        .output()
        .unwrap();
    assert!(output.status.success(), "{}", String::from_utf8_lossy(&output.stderr));
    let text = std::fs::read_to_string(scratch.path().join("a.s")).unwrap();
    let calls = text.lines().map(str::trim).filter(|line| line.starts_with("call ") && line.contains("memcpy")).count();
    assert_eq!(calls, 0, "{text}");
}
