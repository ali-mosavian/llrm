//! -O1 kept three loop-invariant constants as `move.l #imm,%dN` inside a loop
//! that calls out (+9 clk a trip over gcc, which holds them in callee-saved
//! registers): the shared MachineLICM did not run at -O1 (M68K-CALLCLEAN-AREG).

use llrm_m68k::dagisel::hooks::Cpu;
use llrm_m68k::driver::{Level, assembly};

const IR: &str = "target datalayout = \"E-m:e-p:32:16:32-i8:8:8-i16:16:16-i32:16:32-n8:16:32-a:0:16-S16\"
declare i32 @mix(i32, i32)
define i32 @f(i32 %seed) {
b0:
  br label %loop
loop:
  %i = phi i32 [ 0, %b0 ], [ %i1, %loop ]
  %acc = phi i32 [ %seed, %b0 ], [ %r, %loop ]
  %c = and i32 %i, 31
  %a = shl i32 19088743, %c
  %b = lshr i32 1154868983, %c
  %d = shl i32 -1985229329, %c
  %x = or i32 %a, %b
  %y = xor i32 %x, %d
  %r = call i32 @mix(i32 %acc, i32 %y)
  %i1 = add i32 %i, 1
  %e = icmp eq i32 %i1, 64
  br i1 %e, label %done, label %loop
done:
  ret i32 %r
}
";

/// The immediates the loop body loads, from its label to the call.
fn loop_immediates(asm: &str) -> Vec<&str> {
    let body = asm.split("jsr").next().unwrap_or("");
    let body = body.split_once(".LBB0_1:").map_or(body, |(_, rest)| rest);
    body.lines().filter(|line| line.contains("move.l\t#") && !line.contains("#0,")).collect()
}

/// The premise: -O2 holds them before the loop.
#[test]
fn at_o2_the_constants_are_loaded_before_the_loop() {
    let asm = assembly(IR, Cpu::M68020, Level::O2).expect("compiles");
    assert!(loop_immediates(&asm).is_empty(), "the shape changed:\n{asm}");
}

#[test]
fn at_o1_the_constants_are_loaded_before_the_loop() {
    let asm = assembly(IR, Cpu::M68020, Level::O1).expect("compiles");
    assert!(loop_immediates(&asm).is_empty(), "constants reloaded every trip:\n{asm}");
}
