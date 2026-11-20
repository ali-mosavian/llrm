//! A size build divided by a constant with the multiply-high sequence at the
//! 68020 (`move.l #magic; mulu.l; moveq; lsr.l`, 14 B) where one `divu.l`
//! (10 B) does it: the widening multiply made the sequence available there, and
//! seeds 37, 206 and 271 grew by 4 to 22 B at -Os. A speed build keeps the
//! sequence (44 clk against 90).

use llrm_m68k::driver::{Level, assembly, cpu_of};

const DATALAYOUT: &str = "E-m:e-p:32:16:32-i8:8:8-i16:16:16-i32:16:32-n8:16:32-a:0:16-S16";

const DIV: &str = "define i32 @f(i32 %x) {
  %q = udiv i32 %x, 1025
  %r = urem i32 %x, 2049
  %s = add i32 %q, %r
  ret i32 %s
}";

fn text(
    cpu: &str,
    level: Level,
) -> String {
    assembly(&format!("target datalayout = \"{DATALAYOUT}\"\n{DIV}"), cpu_of(cpu).unwrap(), level).expect("compiles")
}

#[test]
fn a_size_build_divides_by_a_constant_with_one_divide() {
    for cpu in ["68020", "68030", "68040"] {
        let out = text(cpu, Level::Os);
        assert!(out.contains("divu.l"), "{cpu}: no divide instruction:\n{out}");
        assert!(!out.contains("mulu.l"), "{cpu}: the multiply-high sequence in a size build:\n{out}");
    }
}

#[test]
fn a_speed_build_keeps_the_multiply_high_sequence() {
    let out = text("68020", Level::O2);
    assert!(out.contains("mulu.l") && !out.contains("divu.l"), "{out}");
}
