//! The function's `"frame-pointer"` attribute decides the frame record
//! (`AArch64FrameLowering::hasFP`, `disableFramePointerElim`): `none` builds no
//! `x29`/`x30` record in a non-leaf function, `all` always does, `non-leaf` does
//! in a non-leaf function and not in a leaf.

use llrm_arm64::driver::{self, Level};
use llrm_arm64::machine::Platform;

const NON_LEAF: &str = include_str!("data/frame_pointer_policy.ll");

fn assembly(
    policy: Option<&str>,
    ir: &str,
) -> String {
    let ir = match policy {
        Some(value) => ir.replace("\"frame-pointer\"=\"non-leaf-no-reserve\"", &format!("\"frame-pointer\"=\"{value}\"")),
        None => ir.replace("\"frame-pointer\"=\"non-leaf-no-reserve\"", ""),
    };
    driver::assembly(&ir, Platform::Linux, Level::Os).expect("lowers")
}

fn records(asm: &str) -> usize {
    asm.lines().filter(|line| line.trim().starts_with("stp x29, x30")).count()
}

/// `-fomit-frame-pointer` (no attribute, or `none`) kept `stp x29, x30` in a
/// non-leaf function, so the bars (gcc, the pin's clang) were frameless against
/// a framed candidate, and `-fno-omit-frame-pointer` changed nothing.
#[test]
fn a_non_leaf_function_with_frame_pointer_none_has_no_frame_record() {
    let asm = assembly(Some("none"), NON_LEAF);
    assert!(asm.contains("bl use") || asm.contains("bl\tuse"), "premise: a non-leaf function:\n{asm}");
    assert_eq!(records(&asm), 0, "{asm}");
    assert!(asm.lines().any(|line| line.contains("x30")), "the link register is saved:\n{asm}");
    assert_eq!(records(&assembly(None, NON_LEAF)), 0, "no attribute is none");
}

#[test]
fn a_function_with_frame_pointer_all_or_non_leaf_has_the_record_when_it_calls() {
    assert_eq!(records(&assembly(Some("all"), NON_LEAF)), 1);
    assert_eq!(records(&assembly(Some("non-leaf"), NON_LEAF)), 1);
    assert_eq!(records(&assembly(Some("non-leaf-no-reserve"), NON_LEAF)), 1);
}
