//! A count of leading or trailing zeros or of set bits was expanded inline (a
//! 216 and a 162 byte function for `clzll` and `ctzll` at `-Os`, 68000); the
//! shared compiler-rt routines are 28 to 78 bytes each. gcc calls libgcc for
//! all six on the 68000 and 68010, and so does this; from the 68020 the
//! count stays inline. The routines are called, not run.

use llrm_m68k::compile::{Options, text_with};
use llrm_m68k::dagisel::hooks::Cpu;

const DATALAYOUT: &str = "E-m:e-p:32:16:32-i8:8:8-i16:16:16-i32:16:32-n8:16:32-a:0:16-S16";

fn text(
    name: &str,
    bits: u32,
    cpu: Cpu,
) -> String {
    let (flag_ty, flag) = if name == "ctpop" { ("", "") } else { (", i1", ", i1 true") };
    let ir = format!(
        "target datalayout = \"{DATALAYOUT}\"\n\
         declare i{bits} @llvm.{name}.i{bits}(i{bits}{flag_ty})\n\
         define i{bits} @f(i{bits} %x) {{ %r = call i{bits} @llvm.{name}.i{bits}(i{bits} %x{flag}) ret i{bits} %r }}"
    );
    let module = llrm_mir::parse::module(&ir).expect("parses");
    text_with(&module, "f", cpu, Options::default()).expect("compiles")
}

#[test]
fn every_count_calls_the_runtime_on_the_68000_and_68010() {
    for cpu in [Cpu::M68000, Cpu::M68010] {
        for (name, bits, routine) in [
            ("ctlz", 32, "__clzsi2"),
            ("ctlz", 64, "__clzdi2"),
            ("cttz", 32, "__ctzsi2"),
            ("cttz", 64, "__ctzdi2"),
            ("ctpop", 32, "__popcountsi2"),
            ("ctpop", 64, "__popcountdi2"),
        ] {
            let out = text(name, bits, cpu);
            assert!(out.contains(&format!("jsr\t{routine}")), "{cpu:?} {name}.i{bits}:\n{out}");
        }
    }
}

/// Our routines cost more clocks than the inline code from the 68020 on (u64
/// -O2: -540 B, +4000 clk, gcc 15542 vs our 21162): no count is a call there
/// until `bfffo` and faster routines exist.
#[test]
fn no_count_is_a_call_from_the_68020_on() {
    for cpu in [Cpu::M68020, Cpu::M68040] {
        for (name, bits) in [("ctlz", 32), ("ctlz", 64), ("cttz", 32), ("cttz", 64), ("ctpop", 32), ("ctpop", 64)] {
            let out = text(name, bits, cpu);
            assert!(
                !out.contains("jsr\t__clz") && !out.contains("jsr\t__ctz") && !out.contains("jsr\t__popcount"),
                "{cpu:?} {name}.i{bits}:\n{out}"
            );
        }
    }
}

fn parity_text(
    bits: u32,
    cpu: Cpu,
) -> String {
    let ir = format!(
        "target datalayout = \"{DATALAYOUT}\"\n\
         declare i{bits} @llvm.ctpop.i{bits}(i{bits})\n\
         define i32 @f(i{bits} %x) {{ %c = call i{bits} @llvm.ctpop.i{bits}(i{bits} %x) %b = and i{bits} %c, 1 %r = zext i{bits} %b to i32 ret i32 %r }}"
    );
    let module = llrm_mir::parse::module(&ir).expect("parses");
    text_with(&module, "f", cpu, Options::default()).expect("compiles")
}

/// `ctpop(x) & 1` is `ISD::PARITY`; the pin's xor tree is 34 bytes of shifts on
/// the 68000 where the call is 12, and bitcount_idioms (parity16 beside ones32,
/// which links `__popcountsi2` anyway) grew 14 bytes at -O1/-O2/-Os when the
/// fold arrived. The count is a call there (BC1), so is its parity.
#[test]
fn the_parity_of_a_word_is_the_count_routine_on_the_68000_and_68010() {
    for cpu in [Cpu::M68000, Cpu::M68010] {
        for bits in [16, 32] {
            let out = parity_text(bits, cpu);
            assert!(out.contains("jsr\t__popcountsi2"), "{cpu:?} parity i{bits}:\n{out}");
            assert!(!out.contains("lsr"), "{cpu:?} parity i{bits} has the xor tree:\n{out}");
        }
    }
}

#[test]
fn the_parity_of_a_word_stays_inline_from_the_68020_on() {
    for cpu in [Cpu::M68020, Cpu::M68040] {
        let out = parity_text(32, cpu);
        assert!(!out.contains("jsr\t__popcount"), "{cpu:?}:\n{out}");
    }
}
