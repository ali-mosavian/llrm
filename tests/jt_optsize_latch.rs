//! At m32 -Os JumpThreading does not copy the outer loop's latch.

mod common;

/// x_radix at -Os kept two `cmp reg, 3` latches (306 B against gcc's 287):
/// the three guarded inner loops let the pin's JumpThreading (copy limit 6)
/// thread the latch block into its own copy. gcc's `-Os` threader refuses a
/// path through a latch; with m32 keeping its latches at size the latch
/// stays one block.
#[test]
fn at_os_the_outer_loop_latch_is_not_copied_by_jump_threading() {
    let source = format!("{}/crates/target/llrm-x86-m32/vsgcc/kernels/x_radix/x_radix.c", env!("CARGO_MANIFEST_DIR"));
    let scratch = tempfile::tempdir().unwrap();
    let output = common::cc::llrm_c()
        .current_dir(scratch.path())
        .args(["-Os", "-m32", "-mabi=sysv", "-march=i486", "-S", "-o", "a.s", &source])
        .output()
        .unwrap();
    assert!(output.status.success(), "{}", String::from_utf8_lossy(&output.stderr));
    let text = std::fs::read_to_string(scratch.path().join("a.s")).unwrap();
    let latches = text.lines().map(str::trim).filter(|line| line.starts_with("cmp ") && line.ends_with(", 3")).count();
    assert_eq!(latches, 1, "{text}");
}
