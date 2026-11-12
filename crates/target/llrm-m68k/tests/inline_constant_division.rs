//! The inliner's byte count of a callee prices `x % K` as the magic multiply a
//! speed caller's DAG makes of it (12 to 38 B), not as one 4 B divide.
//!
//! Before: `recmany_o1` -O2 inlined a six-value loop of three `%` by constants
//! into its caller (growth counted 23 B, real 190 B), 376 B against gcc's 188.

use llrm_m68k::driver::{Level, assembly, cpu_of};
use llrm_m68k::sizes::arithmetic;
use llrm_mir::tti::{ArithOp, OperandInfo, Ty};

const THREE_REMAINDERS: &str = "target datalayout = \"E-m:e-p:32:16:32-i8:8:8-i16:16:16-i32:16:32-n8:16:32-a:0:16-S16\"\n\
define i32 @f(i32 %a, i32 %b, i32 %c) {\n\
  %1 = srem i32 %a, 5\n  %2 = srem i32 %b, 7\n  %3 = srem i32 %c, 3\n  %4 = add i32 %1, %2\n  %5 = add i32 %4, %3\n  ret i32 %5\n}\n\
define i32 @k(i32 %x, i32 %y, i32 %z) {\n  %1 = call i32 @f(i32 %x, i32 %y, i32 %z)\n  %2 = add i32 %1, 1\n  ret i32 %2\n}\n";

fn count_calls(asm: &str) -> usize {
    asm.lines().filter(|line| line.trim_start().starts_with("jsr")).count()
}

#[test]
fn a_speed_caller_keeps_a_call_to_a_callee_of_constant_remainders() {
    let asm = assembly(THREE_REMAINDERS, cpu_of("68020").unwrap(), Level::O2).unwrap_or_else(|why| panic!("{why}"));
    assert_eq!(count_calls(&asm), 1, "{asm}");
}

#[test]
fn a_constant_divisor_is_priced_as_it_is_lowered() {
    let five = OperandInfo::constant(5, 32);
    let price = |op, ty, right, long, speed| arithmetic(op, ty, right, long, speed).value;
    assert_eq!(price(ArithOp::SRem, Ty::Int(32), five, true, true), 38);
    assert_eq!(price(ArithOp::SRem, Ty::Int(32), five, true, false), 4, "a size caller keeps divs.l");
    assert_eq!(price(ArithOp::SRem, Ty::Int(32), five, false, true), 16, "the 68000 calls with the constant pushed");
    assert_eq!(price(ArithOp::SRem, Ty::Int(32), OperandInfo::constant(8, 32), true, false), 4, "so does -O1");
    assert_eq!(price(ArithOp::SDiv, Ty::Int(32), OperandInfo::constant(8, 32), true, true), 18);
    assert_eq!(price(ArithOp::UDiv, Ty::Int(16), five, false, true), 10);
}
