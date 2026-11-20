//! `m = max(m, p[i])` at -Os: the loaded value had to sit in a data register
//! because `cmp.l` has a data destination, the loop needed a third, and
//! `%d2` was saved and restored (arrmax -Os on the 68000: 30 B against gcc's
//! 26). The loaded value may live in `%a1` and be compared with `cmpa.l`.

use llrm_m68k::compile::{Options, text_with};
use llrm_m68k::dagisel::hooks::Cpu;

const MAX: &str = "target datalayout = \"E-m:e-p:32:16:32-i8:8:8-i16:16:16-i32:16:32-n8:16:32-a:0:16-S16\"
define i32 @k(ptr %p, i32 %n) {
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
}
";

#[test]
fn a_compared_value_is_held_in_an_address_register_instead_of_saving_a_data_register() {
    let module = llrm_mir::parse::module(MAX).expect("parses");
    let options = Options { size: true, ..Options::default() };
    let text = text_with(&module, "k", Cpu::M68000, options).expect("compiles");
    assert!(text.contains("cmpa.l"), "{text}");
    assert!(!text.contains("-(%sp)"), "{text}");
}
