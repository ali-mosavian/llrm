//! A stack slot at the stack pointer's own address was `move.l (0,%sp), %d0`:
//! four bytes where `(%sp)` is two (known_loads_walks -Os: 16 reloads, +32 B
//! over gcc's `sp@`).

use llrm_m68k::driver::{Level, assembly, cpu_of};

const IR: &str = "target datalayout = \"E-m:e-p:32:16:32-i8:8:8-i16:16:16-i32:16:32-n8:16:32-a:0:16-S16\"
define i32 @f(i32 %n) {
  %t = alloca [2 x i32]
  %q = getelementptr i32, ptr %t, i32 1
  store volatile i32 %n, ptr %t
  store volatile i32 7, ptr %q
  %a = load volatile i32, ptr %t
  %b = load volatile i32, ptr %q
  %r = add i32 %a, %b
  ret i32 %r
}
";

#[test]
fn a_slot_at_offset_zero_is_register_indirect() {
    for cpu in ["68000", "68020"] {
        for level in [Level::O1, Level::Os] {
            let out = assembly(IR, cpu_of(cpu).unwrap(), level).expect("compiles");
            assert!(!out.contains("(0,"), "{cpu} {level:?}: a zero displacement:\n{out}");
            assert!(out.contains("(%sp)"), "{cpu} {level:?}: no slot at (%sp), the test lost its subject:\n{out}");
        }
    }
}
