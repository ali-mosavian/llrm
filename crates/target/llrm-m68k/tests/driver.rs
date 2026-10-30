//! `driver::assembly` on the IR clang's front end writes for llrm-cc.

use llrm_m68k::dagisel::hooks::Cpu;
use llrm_m68k::driver::{Level, assembly};

const DATALAYOUT: &str = "E-m:e-p:32:16:32-i8:8:8-i16:16:16-i32:16:32-n8:16:32-a:0:16-S16";

/// clang's decorations (`target triple`, `dso_local`, `noundef`) before the mid-end: the unoptimized
/// load/store form of `a + b` must come out as the two-instruction function.
#[test]
fn unoptimized_clang_ir_compiles_to_the_optimized_function() {
    let ir = format!(
        "target datalayout = \"{DATALAYOUT}\"\ntarget triple = \"m68k-unknown-linux-gnu\"\n\
         define dso_local i32 @f(i32 noundef %a, i32 noundef %b) {{\n\
         %a.addr = alloca i32, align 4\n  %b.addr = alloca i32, align 4\n\
         store i32 %a, ptr %a.addr, align 4\n  store i32 %b, ptr %b.addr, align 4\n\
         %x = load i32, ptr %a.addr, align 4\n  %y = load i32, ptr %b.addr, align 4\n\
         %r = add nsw i32 %x, %y\n  ret i32 %r\n}}\n"
    );
    let text = assembly(&ir, Cpu::M68000, Level::O2).expect("compiles");
    assert_eq!(text, "f:\n\tmove.l\t(8,%sp), %d0\n\tadd.l\t(4,%sp), %d0\n\trts\n");
    // the same IR with no mid-end keeps its slots
    let raw = assembly(&ir, Cpu::M68000, Level::O0).expect("compiles");
    assert!(raw.len() > text.len(), "{raw}");
}

/// A module laid out for another machine was compiled as if it were m68k's.
#[test]
fn another_targets_datalayout_is_refused() {
    let ir = "target datalayout = \"e-m:e-p:32:32-i64:64-n8:16:32-S128\"\ndefine i32 @f() {\n  ret i32 0\n}\n";
    let why = assembly(ir, Cpu::M68000, Level::O2).unwrap_err();
    assert!(why.contains("datalayout"), "{why}");
}
