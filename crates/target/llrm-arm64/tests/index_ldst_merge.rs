//! `mov w8, #lo; movk w8, #hi, lsl #16; ldr [x0, x8]` becomes
//! `add x8, x0, #n, lsl #12; ldr [x8, #imm12]`
//! (`AArch64LoadStoreOpt::tryToMergeIndexLdSt`).

use llrm_arm64::driver::{self, Level};
use llrm_arm64::machine::Platform;

fn assembly(
    ty: &str,
    offset: i64,
) -> String {
    let source = format!(
        "target datalayout = \"e-m:e-p270:32:32-p271:32:32-p272:64:64-i8:8:32-i16:16:32-i64:64-i128:128-n32:64-S128-Fn32\"\n\
         target triple = \"aarch64-unknown-linux-gnu\"\n\
         define {ty} @f(ptr %p) {{\n %q = getelementptr i8, ptr %p, i64 {offset}\n %v = load {ty}, ptr %q, align 1\n ret {ty} %v\n}}\n"
    );
    driver::assembly(&source, Platform::Linux, Level::O2).expect("lowers")
}

/// Offset 70000 loaded through `movz; movk; ldr [x0, x9]` (3 instructions
/// before `ret`) where the pin has `add x8, x0, #16, lsl #12; ldr w0, [x8,
/// #4464]` (2).
#[test]
fn a_word_load_at_a_large_offset_takes_the_high_part_in_an_add() {
    let asm = assembly("i32", 70000);
    assert!(asm.contains("#16, lsl #12") && asm.contains("#4464]"), "{asm}");
    assert!(!asm.contains("movk"), "{asm}");
}

/// The same for a byte: the immediate is not scaled, so the split is 17 and
/// 368.
#[test]
fn a_byte_load_at_a_large_offset_splits_at_4096() {
    let asm = assembly("i8", 70000);
    assert!(asm.contains("#17, lsl #12") && asm.contains("#368]"), "{asm}");
}

/// An offset the access size does not divide stays in the register form.
#[test]
fn an_unaligned_offset_keeps_the_index_register() {
    let asm = assembly("i32", 70001);
    assert!(asm.contains("movk"), "{asm}");
}

/// Offsets that need no high part (one `movz`, e.g. 4097 for a byte) were
/// never merged and must not become `add x, x0, #0, lsl #12`: the pin leaves
/// `mov w8, #4097; ldrb w0, [x0, x8]`. A guard: it held before this test.
#[test]
fn an_offset_without_a_high_part_gets_no_add() {
    for (ty, offset) in [("i8", 4097), ("i32", 16388), ("i64", 32776)] {
        let asm = assembly(ty, offset);
        assert!(!asm.lines().any(|line| line.trim_start().starts_with("add")), "{ty} {offset}: {asm}");
        assert!(asm.contains(", x"), "{ty} {offset}: the register form stays: {asm}");
    }
}
