//! #10363 hoisted the `lea` of every global a loop stores to
//! (`word_array_abs_x`) and nothing stopped it: the pin's M68k flags no
//! instruction `isReMaterializable`, so only its register-pressure check
//! (address registers: limit 8) lets a `lea` out, and a `lea` result is an
//! `AR32` value. Here eleven were hoisted, six spilled to the frame and read
//! back every trip (licm_duplicate_phi_input -O2 68000: +70 B, +0.94% clocks;
//! 18 cells slower). The pin hoists seven and spills two.

use llrm_m68k::driver::{Level, assembly, cpu_of};

const STORES: &str = "target datalayout = \"E-m:e-p:32:16:32-i8:8:8-i16:16:16-i32:16:32-n8:16:32-a:0:16-S16\"
@g0 = global i32 0
@g1 = global i32 0
@g2 = global i32 0
@g3 = global i32 0
@g4 = global i32 0
@g5 = global i32 0
@g6 = global i32 0
@g7 = global i32 0
@g8 = global i32 0
@g9 = global i32 0
@g10 = global i32 0
@g11 = global i32 0
define void @f(i32 %n) {
entry:
  br label %loop
loop:
  %i = phi i32 [ 0, %entry ], [ %i1, %loop ]
  store volatile i32 %i, ptr @g0
  store volatile i32 %i, ptr @g1
  store volatile i32 %i, ptr @g2
  store volatile i32 %i, ptr @g3
  store volatile i32 %i, ptr @g4
  store volatile i32 %i, ptr @g5
  store volatile i32 %i, ptr @g6
  store volatile i32 %i, ptr @g7
  store volatile i32 %i, ptr @g8
  store volatile i32 %i, ptr @g9
  store volatile i32 %i, ptr @g10
  store volatile i32 %i, ptr @g11
  %i1 = add i32 %i, 1
  %c = icmp eq i32 %i1, %n
  br i1 %c, label %done, label %loop
done:
  ret void
}
";

#[test]
fn symbol_addresses_are_hoisted_only_while_address_registers_last() {
    let out = assembly(STORES, cpu_of("68000").unwrap(), Level::O2).expect("compiles");
    let spilled =
        out.lines().filter(|line| line.contains("move.l") && line.contains("%a") && line.contains(",%sp)")).count();
    assert!(spilled <= 4, "{spilled} stores and reloads of an address through the frame (the pin has 4):\n{out}");
}
