//! The function's `"frame-pointer"` attribute decides the frame record
//! (`AArch64FrameLowering::hasFP`, `disableFramePointerElim`): `none` builds no
//! `x29`/`x30` record in a non-leaf function, `all` always does, `non-leaf` does
//! in a non-leaf function and not in a leaf.

use llrm_arm64::driver::{self, Level};
use llrm_arm64::machine::Platform;

const LAYOUT: &str = "target datalayout = \"e-m:e-p270:32:32-p271:32:32-p272:64:64-i8:8:32-i16:16:32-i64:64-i128:128-n32:64-S128-Fn32\"\ntarget triple = \"aarch64-unknown-linux-gnu\"\n";
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

/// `hasFP` also holds for a variable-sized object whatever the attribute says.
#[test]
fn a_variable_sized_object_keeps_the_record_under_frame_pointer_none() {
    let ir = format!("{LAYOUT}declare void @use(ptr)\ndefine void @f(i64 %n) \"frame-pointer\"=\"none\" {{\n  %a = alloca i8, i64 %n, align 16\n  call void @use(ptr %a)\n  ret void\n}}\n");
    assert_eq!(records(&driver::assembly(&ir, Platform::Linux, Level::O2).expect("lowers")), 1);
}

/// `hasFP` holds for a call frame over `DefaultSafeSPDisplacement` (255 bytes):
/// the frame is the stack arguments rounded up to 16: 33 arguments are 272, 31 are 256, 30 are 240.
#[test]
fn a_call_frame_over_255_bytes_keeps_the_record_under_frame_pointer_none() {
    let call = |stack: usize| {
        let arguments = (0..8 + stack).map(|n| format!("i64 {n}")).collect::<Vec<_>>().join(", ");
        let parameters = (0..8 + stack).map(|_| "i64").collect::<Vec<_>>().join(", ");
        let ir = format!("{LAYOUT}declare void @g({parameters})\ndefine void @f() \"frame-pointer\"=\"none\" {{\n  call void @g({arguments})\n  ret void\n}}\n");
        records(&driver::assembly(&ir, Platform::Linux, Level::O2).expect("lowers"))
    };
    assert_eq!((call(33), call(31), call(30)), (1, 1, 0));
}

/// Darwin: clang's `all` gives the record; the pin saves the frame pointer
/// beside the link register there whatever the attribute (FP-DARWIN).
#[test]
fn darwin_keeps_the_record_for_all_and_for_a_call() {
    let darwin = |value: &str| {
        let ir = NON_LEAF
            .replace("\"frame-pointer\"=\"non-leaf-no-reserve\"", &format!("\"frame-pointer\"=\"{value}\""))
            .replace("e-m:e-p270:32:32-p271:32:32-p272:64:64-i8:8:32-i16:16:32-i64:64-i128:128-n32:64-S128-Fn32", "e-m:o-p270:32:32-p271:32:32-p272:64:64-i64:64-i128:128-n32:64-S128-Fn32")
            .replace("aarch64-unknown-linux-gnu", "arm64-apple-macosx");
        let asm = driver::assembly(&ir, Platform::Darwin, Level::Os).expect("lowers");
        asm.lines().filter(|line| line.trim().starts_with("stp x29, x30")).count()
    };
    assert_eq!((darwin("all"), darwin("none")), (1, 1));
}
