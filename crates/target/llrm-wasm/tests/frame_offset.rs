use std::path::Path;
use std::process::Command;

/// A plain `getelementptr i8, ptr %alloca, 4` kept its `i32.add`: a plain
/// `add` is no `nuw`, so nothing folded it into `offset=`. fpbench -O2 made 55
/// `i32.add` (the pin 34): the aligned frame object makes the `add` an
/// `or disjoint`, which folds.
#[test]
fn a_frame_offset_inside_the_alignment_folds_into_the_access() {
    let ir = Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/ir/frame_offset.ll");
    let out = Command::new(env!("CARGO_BIN_EXE_llrm-wasm-llc"))
        .args(["-S", "-O2", "-o", "-"])
        .arg(ir)
        .output()
        .expect("llrm-wasm-llc");
    assert!(out.status.success());
    let adds = String::from_utf8_lossy(&out.stdout).lines().filter(|line| line.trim() == "i32.add").count();
    assert!(adds <= 34, "{adds} i32.add");
}
