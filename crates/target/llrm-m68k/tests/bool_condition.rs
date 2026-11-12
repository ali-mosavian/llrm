//! A select on an `i1` phi (`true` on the first trip, `false` after) tested
//! `and.b #1; neg.b; tst.b`: the promoted condition is `sign_extend_inreg x,
//! i1`, and the `neg` made the whole byte -1 or 0 to compare it with zero
//! (float_in_place_update -Os 164 -> 202 B once the compare of a constant
//! phi folds). Bit 0 of `x` is the condition.

use llrm_m68k::compile::{Options, text_with};
use llrm_m68k::dagisel::hooks::Cpu;

const LOOP: &str = "target datalayout = \"E-m:e-p:32:16:32-i8:8:8-i16:16:16-i32:16:32-n8:16:32-a:0:16-S16\"
declare i32 @g(i32)
define i32 @f(i32 %n, i32 %a, i32 %b) {
entry:
  br label %loop
loop:
  %i = phi i32 [0, %entry], [%i1, %loop]
  %acc = phi i32 [%a, %entry], [%r, %loop]
  %first = phi i1 [true, %entry], [false, %loop]
  %v = call i32 @g(i32 %acc)
  %r = select i1 %first, i32 %b, i32 %v
  %i1 = add i32 %i, 1
  %c = icmp ne i32 %i1, %n
  br i1 %c, label %loop, label %done
done:
  ret i32 %r
}
";

#[test]
fn a_select_on_a_bool_phi_tests_its_bit_without_negating_it() {
    let module = llrm_mir::parse::module(LOOP).expect("parses");
    for cpu in [Cpu::M68000, Cpu::M68020] {
        let out = text_with(&module, "f", cpu, Options { size: true, mid_end: false, ..Options::default() })
            .expect("compiles");
        assert!(!out.contains("neg.b"), "{cpu:?}: the condition is negated:\n{out}");
    }
}

/// The exit of float_in_place_update once the compare of a constant phi folds
/// (`br i1 %6, %exit, %loop` with `%6 = phi i1 [false], [true]`) branched on
/// `not.b; and.b #1`: the opposite condition is the same bit tested the other
/// way, not a copy of the byte made opposite first.
const EXIT_PHI: &str = include_str!("fixtures/bool_phi_exit_main.ll");

#[test]
fn a_branch_on_the_opposite_of_a_bool_tests_the_bool_with_the_opposite_condition() {
    let module = llrm_mir::parse::module(EXIT_PHI).expect("parses");
    let out = text_with(&module, "main", Cpu::M68000, Options { size: true, mid_end: false, ..Options::default() })
        .expect("compiles");
    assert!(!out.contains("not.b"), "the boolean is inverted first:\n{out}");
}
