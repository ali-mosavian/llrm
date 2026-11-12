//! `abs` took the shift form `(x ^ (x >> 31)) - (x >> 31)`: on the 68000 a
//! shift by 31 is a register count (`asr.l` takes 1 to 8 as an immediate,
//! 6 + 2n clocks) and the sign lives in a second register. gcc compares and
//! negates (`tst`, `bpl`, `neg`).

use llrm_m68k::driver::{Level, assembly, cpu_of};

const ABS: &str = "
target datalayout = \"E-m:e-p:32:16:32-i8:8:8-i16:16:16-i32:16:32-n8:16:32-a:0:16-S16\"
target triple = \"m68k-unknown-linux-gnu\"
declare i32 @llvm.abs.i32(i32, i1)
define i32 @f(i32 %a, i32 %b) {
  %x = call i32 @llvm.abs.i32(i32 %a, i1 false)
  %y = call i32 @llvm.abs.i32(i32 %b, i1 false)
  %d = sub i32 %x, %y
  ret i32 %d
}
";

#[test]
fn an_abs_is_a_compare_and_a_negate_not_a_shift_by_the_width() {
    for cpu in ["68000", "68020"] {
        for level in [Level::O2, Level::Os] {
            let out = assembly(ABS, cpu_of(cpu).unwrap(), level).expect("compiles");
            assert!(!out.contains("asr"), "{cpu}: the sign is shifted out:\n{out}");
            assert!(!out.contains("eor"), "{cpu}: the sign is xored in:\n{out}");
            assert!(out.contains("neg.l"), "{cpu}: no negate:\n{out}");
        }
    }
}
