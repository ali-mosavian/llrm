//! A count of leading or trailing zeros or of set bits was expanded inline (a 216 and a 162 byte
//! function for `clzll` and `ctzll` at `-Os`, 68000); the shared compiler-rt routines are 28 to 78
//! bytes each. The routines are called, not run.

use llrm_m68k::compile::{Options, text_with};
use llrm_m68k::dagisel::hooks::Cpu;

const DATALAYOUT: &str = "E-m:e-p:32:16:32-i8:8:8-i16:16:16-i32:16:32-n8:16:32-a:0:16-S16";

#[test]
fn a_count_of_a_word_or_a_double_word_calls_the_runtime() {
    for (name, bits, routine) in [
        ("ctlz", 32, "__clzsi2"),
        ("ctlz", 64, "__clzdi2"),
        ("cttz", 32, "__ctzsi2"),
        ("cttz", 64, "__ctzdi2"),
        ("ctpop", 32, "__popcountsi2"),
        ("ctpop", 64, "__popcountdi2"),
    ] {
        let flag = if name == "ctpop" { "" } else { ", i1 true" };
        let ir = format!(
            "target datalayout = \"{DATALAYOUT}\"\n\
             declare i{bits} @llvm.{name}.i{bits}(i{bits}{flag_ty})\n\
             define i{bits} @f(i{bits} %x) {{ %r = call i{bits} @llvm.{name}.i{bits}(i{bits} %x{flag}) ret i{bits} %r }}",
            flag_ty = if name == "ctpop" { "" } else { ", i1" },
        );
        let module = llrm_mir::parse::module(&ir).expect("parses");
        for cpu in [Cpu::M68000, Cpu::M68020] {
            let out = text_with(&module, "f", cpu, Options::default()).expect("compiles");
            assert!(out.contains(&format!("jsr\t{routine}")), "{cpu:?} {name}.i{bits}:\n{out}");
        }
    }
}
