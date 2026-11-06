//! A pointer read in a loop header and stepped in the latch, with a store
//! through the old pointer in a conditional block between, kept `move.l (%a0),
//! %d1` and `addq.l #4, %a0` (the pin's LSR and the per-block indexed combine
//! both stop at the store): 4 B and 8 clocks (68000) more per trip than gcc's
//! `(%a0)+`.

use llrm_m68k::compile::{Options, text_with};
use llrm_m68k::dagisel::hooks::Cpu;

const DATALAYOUT: &str = "E-m:e-p:32:16:32-i8:8:8-i16:16:16-i32:16:32-n8:16:32-a:0:16-S16";

/// `f(p, n)`: sum the elements below 5; each such element is overwritten by
/// the running sum, through the pointer the loop read it by. The element is read
/// twice, so the load stays a `move`.
const STORE_THROUGH_THE_SAME_POINTER: &str = "define i32 @f(ptr %p, i32 %n) {
entry:
  br label %h
h:
  %i = phi i32 [ 0, %entry ], [ %i2, %l ]
  %c = phi i32 [ 0, %entry ], [ %c2, %l ]
  %g = getelementptr [4 x i8], ptr %p, i32 %i
  %v = load i32, ptr %g, align 4
  %lt = icmp slt i32 %v, 5
  br i1 %lt, label %t, label %l
t:
  store i32 %c, ptr %g, align 4
  %c1 = add i32 %c, %v
  br label %l
l:
  %c2 = phi i32 [ %c1, %t ], [ %c, %h ]
  %i2 = add nuw nsw i32 %i, 1
  %e = icmp eq i32 %i2, %n
  br i1 %e, label %x, label %h
x:
  ret i32 %c2
}";

fn asm(
    cpu: Cpu,
    options: Options,
) -> String {
    let module =
        llrm_mir::parse::module(&format!("target datalayout = \"{DATALAYOUT}\"\n{STORE_THROUGH_THE_SAME_POINTER}"))
            .expect("parses");
    text_with(&module, "f", cpu, options).expect("compiles")
}

/// The load is `move.l (%aN)+` and no `addq.l #4` steps the pointer, at every
/// CPU; the store through the old pointer reads `(-4,%aN)`.
#[test]
fn the_pointer_is_stepped_by_the_load_not_by_a_separate_addq() {
    for cpu in [Cpu::M68000, Cpu::M68020, Cpu::M68040, Cpu::M68060] {
        let text = asm(cpu, Options::default());
        assert!(text.contains(")+, %d"), "{cpu:?}: no post-increment load\n{text}");
        assert!(!text.contains("addq.l\t#4, %a"), "{cpu:?}: the pointer is still stepped apart\n{text}");
        assert!(text.contains("(-4,%a"), "{cpu:?}: the store does not read back from the stepped pointer\n{text}");
    }
}

/// The pin's own output keeps the step: the pass is off for `Options::PINNED`.
#[test]
fn the_option_off_keeps_the_separate_step() {
    let text = asm(Cpu::M68020, Options { post_inc_across_blocks: false, ..Options::default() });
    assert!(text.contains("addq.l\t#4, %a"), "the step vanished with the pass off\n{text}");
}
