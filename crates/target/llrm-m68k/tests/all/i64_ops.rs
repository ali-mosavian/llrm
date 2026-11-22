//! A double-word divide was `Unsupported { "the i64 divide routine is
//! unavailable" }` and a float to `i64` a panic in `RetCC_M68k_C` (an 8-byte
//! result): the target named no routine for the first and passed the second's
//! result as one value. The routines are called, not run.

use llrm_m68k::compile::{Options, text_with};
use llrm_m68k::dagisel::hooks::Cpu;

const DATALAYOUT: &str = "E-m:e-p:32:16:32-i8:8:8-i16:16:16-i32:16:32-n8:16:32-a:0:16-S16";

fn module(ir: &str) -> llrm_mir::module::Module {
    llrm_mir::parse::module(&format!("target datalayout = \"{DATALAYOUT}\"\n{ir}")).expect("parses")
}

/// The routines the pin calls for a double-word divide and a float conversion.
#[test]
fn double_word_divides_and_remainders_call_the_runtime() {
    for (op, routine) in [("sdiv", "__divdi3"), ("udiv", "__udivdi3"), ("srem", "__moddi3"), ("urem", "__umoddi3")] {
        let ir = format!("define i64 @f(i64 %a, i64 %b) {{ %r = {op} i64 %a, %b ret i64 %r }}");
        for cpu in [Cpu::M68000, Cpu::M68020] {
            let out = text_with(&module(&ir), "f", cpu, Options::default()).expect("compiles");
            assert!(out.contains(&format!("jsr\t{routine}")), "{cpu:?} {op}:\n{out}");
        }
    }
}

/// `fptosi float to i64` is `__fixsfdi`: the result is the high word in `d0`
/// and the low in `d1`, not one 8-byte value.
#[test]
fn a_float_to_i64_calls_the_runtime_and_keeps_both_words() {
    for (op, routine) in [("fptosi", "__fixsfdi"), ("fptoui", "__fixunssfdi")] {
        let ir = format!("define i64 @f(float %a) {{ %r = {op} float %a to i64 ret i64 %r }}");
        for cpu in [Cpu::M68000, Cpu::M68020] {
            let out = text_with(&module(&ir), "f", cpu, Options::default()).expect("compiles");
            assert!(out.contains(&format!("jsr\t{routine}")), "{cpu:?} {op}:\n{out}");
        }
    }
}

/// m68k names `__muldi3` but no routine for a function tuned for size, and
/// expanded a double-word multiply inline on the 68000. The legalizer called
/// the routine for every target that named one, so the multiply became a call
/// (2 of 4 programs of a wide-multiply identity run changed).
#[test]
fn a_double_word_multiply_stays_inline_on_the_68000() {
    let ir = "define i64 @f(i64 %a, i64 %b) { %r = mul i64 %a, %b ret i64 %r }";
    let options = Options { call_targets: false, ..Options::default() };
    let out = text_with(&module(ir), "f", Cpu::M68000, options).expect("compiles");
    assert!(out.contains("jsr\t__mulsi3"), "no multiply at all, nothing was tested:\n{out}");
    assert!(!out.contains("__muldi3"), "{out}");
}

/// A double-word divide and remainder of the same operands were one `__divdi3` and a multiply back (`x - (x / y) * y`:
/// two `__mulsi3` calls and a subtract on the 68000, int64_divmod_edges +66 B at -O1): the mid-end's DivRemPairs
/// rewrites the remainder where `has_div_rem_op(64)` is false. They are one call where the multiply back is not the
/// 68020's `muls.l`; where it is, the pin's form stays.
#[test]
fn a_double_word_divide_and_remainder_of_the_same_operands_are_one_call() {
    use llrm_m68k::driver::{Level, assembly};
    for (ops, routine, alone) in
        [(["sdiv", "srem"], "__divmoddi4", "__divdi3"), (["udiv", "urem"], "__udivmoddi4", "__udivdi3")]
    {
        let ir = format!(
            "target datalayout = \"{DATALAYOUT}\"\ndefine i64 @f(i64 %a, i64 %b) {{ %q = {} i64 %a, %b %r = {} i64 %a, %b %s = add i64 %q, %r ret i64 %s }}",
            ops[0], ops[1]
        );
        for level in [Level::O1, Level::O2, Level::Os] {
            for cpu in [Cpu::M68000, Cpu::M68010, Cpu::M68060] {
                let out = assembly(&ir, cpu, level).expect("compiles");
                assert_eq!(out.matches("jsr").count(), 1, "{cpu:?} {level:?} {ops:?}:\n{out}");
                assert!(out.contains(&format!("jsr\t{routine}")), "{cpu:?} {level:?} {ops:?}:\n{out}");
            }
            let out = assembly(&ir, Cpu::M68020, level).expect("compiles");
            assert!(out.contains(&format!("jsr\t{alone}")) && !out.contains(routine), "68020 {level:?} {ops:?}:\n{out}");
        }
    }
}
