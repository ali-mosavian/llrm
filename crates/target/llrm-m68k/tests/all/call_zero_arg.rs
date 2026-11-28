//! A call whose second stack argument is the constant 0 kept the reserved
//! call frame (`lea (-8,%sp),%sp; move.l %d0,(%sp); clr.l (4,%sp)`, 12 B)
//! because the `clr.l` store was not known as an argument store; the other
//! constants were pushed. `__unordsf2(x, 0.0)` made `canon` 6 B bigger at
//! 68020 -Os (sfm16 +6 B).

use llrm_m68k::driver::{Level, assembly, cpu_of};

const ZERO_ARG: &str = "target datalayout = \"E-m:e-p:32:16:32-i8:8:8-i16:16:16-i32:16:32-n8:16:32-a:0:16-S16\"
declare i32 @ext(i32, i32)
define i32 @g(i32 %x) {
  %r = call i32 @ext(i32 %x, i32 0)
  ret i32 %r
}";

/// At speed the zero is `pea 0:w`: 6 clk faster than `clr.l -(%sp)` on the
/// 68000.
#[test]
fn a_zero_argument_is_pushed_as_pea_at_o2() {
    for cpu in ["68000", "68020", "68040", "68060"] {
        let out = assembly(ZERO_ARG, cpu_of(cpu).unwrap(), Level::O2).expect("compiles");
        assert!(out.contains("pea\t0:w"), "{cpu}: the zero is not pushed with pea\n{out}");
        assert!(!out.contains("lea\t(-8,%sp)"), "{cpu}: the call area is still reserved\n{out}");
    }
}

#[test]
fn a_zero_argument_is_pushed_like_any_other_constant() {
    for cpu in ["68000", "68020", "68040", "68060"] {
        let out = assembly(ZERO_ARG, cpu_of(cpu).unwrap(), Level::Os).expect("compiles");
        assert!(out.contains("clr.l\t-(%sp)"), "{cpu}: the zero is not pushed with clr.l\n{out}");
        assert!(!out.contains("lea\t(-8,%sp)"), "{cpu}: the call area is still reserved\n{out}");
        assert!(!out.contains("%sp, -(%sp)"), "{cpu}: the stack pointer is pushed as the argument\n{out}");
    }
}
