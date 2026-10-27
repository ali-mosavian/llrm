//! The pin orders the stack objects of a frame by uses per byte
//! (`X86FrameLowering::orderFrameObjects`), so what a function touches often is
//! within a one-byte displacement of BP and the arrays lie deep.

use std::process::Command;

/// `LOCALS` of bounds.bas (qb, m16 -Os, `--isel=dag`) holds two 46-byte array
/// descriptors and 24 words; laid in the order they were made, 105 of its 132
/// `[bp-N]` operands were four-byte displacements (N > 128), +60 bytes over the
/// old selector. Ordered by use, the loop counters and temporaries are near.
#[test]
fn test_a_frame_with_arrays_keeps_what_is_used_often_within_a_byte_of_bp() {
    let dir = tempfile::tempdir().unwrap();
    let out = dir.path().join("bounds.s");
    let done = Command::new(env!("CARGO_BIN_EXE_llrm-qb"))
        .arg(format!("{}/tests/run/qb/bounds.bas", env!("CARGO_MANIFEST_DIR")))
        .args(["--dialect", "qb45", "--runtime", "qb45", "-Os", "-m16", "-march=i486", "--isel=dag", "-S", "-o"])
        .arg(&out)
        .output()
        .unwrap();
    assert!(done.status.success(), "{}", String::from_utf8_lossy(&done.stderr));
    let listing = std::fs::read_to_string(out).unwrap();
    let locals: Vec<&str> = listing.lines().skip_while(|line| !line.starts_with("LOCALS proc")).collect();
    let locals = &locals[..locals.iter().position(|line| line.starts_with("LOCALS endp")).expect("endp")];
    let (mut near, mut far) = (0, 0);
    for line in locals {
        for cell in line.split("[bp-").skip(1) {
            match cell.split(']').next().and_then(|n| n.parse::<u32>().ok()) {
                Some(n) if n > 128 => far += 1,
                Some(_) => near += 1,
                None => {}
            }
        }
    }
    assert!(far < 30, "{far} of {} frame operands are past [bp-128]", near + far);
    assert!(near > 100, "premise: the loop's cells are frame operands ({near} near, {far} far)");
}
