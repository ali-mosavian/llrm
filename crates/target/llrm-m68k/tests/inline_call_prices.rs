//! The m68k prices of a call for the inliner (`Tti::call_sequence_cost`,
//! `callee_frame_cost`, `opt_size_inline_by_code_size`), and the -Os bound they
//! feed: a callee that stays must not grow the code.

use llrm_m68k::dagisel::hooks::Cpu;
use llrm_m68k::driver::{Level, assembly, cpu_of};
use llrm_m68k::sizes::{call_sequence, callee_frame};
use llrm_mir::tti::{CallArgument, CallShape, CostKind, Ty};

fn shape(
    words: usize,
    frame: u64,
) -> CallShape {
    CallShape {
        arguments: vec![CallArgument { ty: Some(Ty::Int(32)), byval_bytes: None }; words],
        callee_frame_bytes: frame,
        caller_frame_bytes: 0,
    }
}

/// `jsr` 6, one `move.l (4,%sp),-(%sp)` of 4 per word, the `addq`/`lea` that
/// pops them, the callee's `rts` 2.
#[test]
fn a_call_is_its_jsr_pushes_cleanup_and_rts() {
    let bytes = |words| call_sequence(Cpu::M68000, &shape(words, 0), CostKind::CodeSize).value;
    assert_eq!(bytes(0), 8);
    assert_eq!(bytes(2), 18);
    assert_eq!(bytes(3), 24);
}

#[test]
fn a_callee_with_locals_pays_link_and_unlk() {
    assert_eq!(callee_frame(Cpu::M68000, &shape(1, 8), CostKind::CodeSize).map(|cost| cost.value), Some(6));
    assert_eq!(callee_frame(Cpu::M68000, &shape(1, 0), CostKind::CodeSize), None);
}

/// Ten arithmetic operations (40 B) called from three places: inlined three
/// times they are 120 B against 40 + 3 calls of 18 B. A size build kept all
/// three inlined (0 `jsr`) before the bound counted bytes.
const THREE_SITES: &str = "target datalayout = \"E-m:e-p:32:16:32-i8:8:8-i16:16:16-i32:16:32-n8:16:32-a:0:16-S16\"\n\
define internal i32 @f(i32 %a, i32 %b) {\n\
  %1 = add i32 %a, 1000\n  %2 = xor i32 %1, %b\n  %3 = sub i32 %2, 7000\n  %4 = and i32 %3, 4095\n\
  %5 = or i32 %4, %a\n  %6 = add i32 %5, 300\n  %7 = xor i32 %6, %b\n  %8 = sub i32 %7, 9000\n\
  %9 = and i32 %8, 8191\n  %10 = or i32 %9, %b\n  ret i32 %10\n}\n\
define i32 @k(i32 %x, i32 %y) {\n\
  %1 = call i32 @f(i32 %x, i32 %y)\n  %2 = call i32 @f(i32 %1, i32 %x)\n  %3 = call i32 @f(i32 %2, i32 %y)\n  ret i32 %3\n}\n";

#[test]
fn a_size_build_keeps_a_callee_that_would_grow_the_code_as_a_call() {
    let asm = assembly(THREE_SITES, cpu_of("68000").unwrap(), Level::Os).unwrap_or_else(|why| panic!("{why}"));
    // a callee called three times may be called through a register: count the
    // calls, not the name
    assert_eq!(asm.lines().filter(|line| line.trim_start().starts_with("jsr")).count(), 3, "{asm}");
}
