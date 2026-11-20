//! The pin's early MachineLICM hoists the address of a global out of a loop. m68k kept
//! `lea (l,%pc), %a0` in the loop: `word_array_abs_x` -O2 ran 32657 clk at 68020 against
//! gcc's 26266 with two `lea`s a trip.

use llrm_m68k::driver::{Level, assembly, cpu_of};

const LOOP: &str = r#"target datalayout = "E-m:e-p:32:16:32-i8:8:8-i16:16:16-i32:16:32-n8:16:32-a:0:16-S16"
target triple = "m68k-unknown-linux-gnu"

@l = global [64 x i32] zeroinitializer, align 4

define void @fill(i32 %n) nounwind {
entry:
  br label %loop
loop:
  %i = phi i32 [ 0, %entry ], [ %next, %loop ]
  %masked = and i32 %i, 63
  %slot = getelementptr inbounds [64 x i32], ptr @l, i32 0, i32 %masked
  store i32 %i, ptr %slot, align 4
  %next = add nuw nsw i32 %i, 1
  %done = icmp eq i32 %next, %n
  br i1 %done, label %exit, label %loop
exit:
  ret void
}
"#;

fn position(
    asm: &str,
    needle: &str,
) -> usize {
    asm.lines().position(|line| line.contains(needle)).unwrap_or_else(|| panic!("no {needle}:\n{asm}"))
}

#[test]
fn the_address_of_a_global_is_made_before_the_loop_at_speed() {
    for cpu in ["68000", "68020"] {
        for level in [Level::O2, Level::O3] {
            let asm = assembly(LOOP, cpu_of(cpu).unwrap(), level).expect("compiles");
            assert!(position(&asm, "lea") < position(&asm, ".LBB0_1:"), "{cpu} {level:?}: the lea is in the loop:\n{asm}");
        }
    }
}
