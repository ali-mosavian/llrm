//! arrmax at -Os kept the running maximum in a data register: the long loaded
//! through `(An)+` was selected as the data-register row, so three values
//! needed data registers and the third took `d2`, a push and a pop (34 B, gcc
//! 26 B). The load takes either kind of register and the maximum, only
//! compared and copied, lives in an address register.

use llrm_m68k::compile::{Options, text_with};
use llrm_m68k::dagisel::hooks::Cpu;

const DATALAYOUT: &str = "E-m:e-p:32:16:32-i8:8:8-i16:16:16-i32:16:32-n8:16:32-a:0:16-S16";

const ARRMAX: &str = "define i32 @f(ptr %p, i32 %n) {
entry:
  %f = load i32, ptr %p
  br label %loop
loop:
  %i = phi i32 [1, %entry], [%i1, %loop]
  %m = phi i32 [%f, %entry], [%m1, %loop]
  %q = getelementptr i32, ptr %p, i32 %i
  %v = load i32, ptr %q
  %c = icmp sgt i32 %v, %m
  %m1 = select i1 %c, i32 %v, i32 %m
  %i1 = add i32 %i, 1
  %d = icmp slt i32 %i1, %n
  br i1 %d, label %loop, label %exit
exit:
  ret i32 %m1
}";

#[test]
fn a_running_maximum_needs_no_saved_register() {
    let module = llrm_mir::parse::module(&format!("target datalayout = \"{DATALAYOUT}\"\n{ARRMAX}")).expect("parses");
    for cpu in [Cpu::M68000, Cpu::M68020, Cpu::M68040] {
        let text = text_with(&module, "f", cpu, Options { size: true, ..Options::default() }).expect("compiles");
        assert!(!text.contains("-(%sp)"), "{cpu:?}: a register is saved\n{text}");
        assert!(text.contains(")+, %a"), "{cpu:?}: no load into an address register\n{text}");
    }
}
