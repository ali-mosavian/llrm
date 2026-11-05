//! A loop bounded by a stack argument kept it in a register for the whole of
//! the loop (`move.l 4(%sp), %a1` and `cmp.l %a1, %d2`, a saved `%d2` as
//! well): 30 B against gcc's 22 B (bench/kernels/fib). The compare reads the
//! slot.

use llrm_m68k::compile::{Options, text_with};
use llrm_m68k::dagisel::hooks::Cpu;

const LOOP: &str = r#"target datalayout = "E-m:e-p:32:16:32-i8:8:8-i16:16:16-i32:16:32-n8:16:32-a:0:16-S16"
define i32 @k(i32 %n) {
entry:
  br label %loop
loop:
  %i = phi i32 [0, %entry], [%i1, %loop]
  %a = phi i32 [0, %entry], [%b, %loop]
  %b = phi i32 [1, %entry], [%s, %loop]
  %s = add i32 %a, %b
  %i1 = add i32 %i, 1
  %c = icmp slt i32 %i1, %n
  br i1 %c, label %loop, label %exit
exit:
  ret i32 %s
}"#;

fn compile(
    cpu: Cpu,
    size: bool,
) -> String {
    let module = llrm_mir::parse::module(LOOP).expect("parses");
    text_with(&module, "k", cpu, Options { size, ..Options::default() }).expect("compiles")
}

#[test]
fn a_loop_bound_in_a_stack_argument_is_compared_in_place_at_a_smaller_size() {
    for cpu in [Cpu::M68000, Cpu::M68020] {
        let out = compile(cpu, true);
        assert!(out.contains("cmp.l\t(8,%sp)"), "{cpu:?}: the argument was loaded first:\n{out}");
        assert!(!out.contains("(8,%sp), %a"), "{cpu:?}: a register holds the argument:\n{out}");
    }
}

#[test]
fn the_same_loop_keeps_the_argument_in_a_register_at_speed() {
    let out = compile(Cpu::M68000, false);
    assert!(!out.contains("cmp.l\t(8,%sp)"), "a memory compare in the loop at speed:\n{out}");
}
