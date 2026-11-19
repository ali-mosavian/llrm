//! strlen's loop was `move.b (%a0)+,%d1; addq.l #1,%d0; tst.b %d1; bne`: the
//! `move.b` already set the codes, but the counter's `addq` between them kept
//! the `tst` (one instruction a trip over gcc; 68060: 52 clocks against 38).
//! The independent setter moves above the load and the `tst` goes.

use llrm_m68k::compile::{Options, text_with};
use llrm_m68k::dagisel::hooks::Cpu;

const STRLEN: &str = include_str!("../bench/kernels/strlen.ll");

/// Sums the bytes it walks: the `add` reads the loaded byte, so it cannot move
/// above the load and the `tst` must stay.
const SUM_BYTES: &str = "target datalayout = \"E-m:e-p:32:16:32-i8:8:8-i16:16:16-i32:16:32-n8:16:32-a:0:16-S16\"
define i32 @k(ptr %p) {
entry:
  br label %l
l:
  %i = phi i32 [ 0, %entry ], [ %i1, %l ]
  %s = phi i32 [ 0, %entry ], [ %s1, %l ]
  %q = getelementptr i8, ptr %p, i32 %i
  %c = load i8, ptr %q
  %w = zext i8 %c to i32
  %s1 = add i32 %s, %w
  %i1 = add i32 %i, 1
  %z = icmp eq i8 %c, 0
  br i1 %z, label %x, label %l
x:
  ret i32 %s1
}";

fn asm(
    ir: &str,
    cpu: Cpu,
    hoist: bool,
) -> String {
    let module = llrm_mir::parse::module(ir).expect("parses");
    text_with(&module, "k", cpu, Options { hoist_flag_setters: hoist, ..Options::default() }).expect("compiles")
}

#[test]
fn the_counter_moves_above_the_load_and_the_tst_goes() {
    for cpu in [Cpu::M68000, Cpu::M68020, Cpu::M68060] {
        let kept = asm(STRLEN, cpu, false);
        assert!(kept.contains("tst.b"), "premise: the loop keeps its tst without the pass:\n{kept}");
        let out = asm(STRLEN, cpu, true);
        assert!(!out.contains("tst.b"), "{cpu:?}:\n{out}");
    }
}

#[test]
fn a_setter_that_reads_the_loaded_byte_keeps_the_tst() {
    let out = asm(SUM_BYTES, Cpu::M68000, true);
    assert!(out.contains("tst."), "{out}");
}
