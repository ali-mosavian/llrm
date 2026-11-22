//! Constrained `f32`/`f64` conversions on arm64 select the instruction, as the
//! pin's llc does (the `STRICT_*` conversions are `Custom` and then match
//! `scvtf`/`ucvtf`/`fcvtz*`/`fcvt`). Before, each was a compiler-rt routine
//! (`bl __floatsidf`, `__fixdfsi`, `__extendsfdf2`, `__truncdfsf2`).

use llrm_arm64::driver;
use llrm_arm64::machine::Platform;

const LAYOUT: &str = "target datalayout = \"e-m:e-p270:32:32-p271:32:32-p272:64:64-i8:8:32-i16:16:32-i64:64-i128:128-n32:64-S128-Fn32\"\n";

#[test]
fn constrained_conversions_are_instructions_not_routines() {
    let cases = [
        ("sitofp", "i32", "double", "scvtf"),
        ("sitofp", "i64", "float", "scvtf"),
        ("uitofp", "i32", "float", "ucvtf"),
        ("uitofp", "i64", "double", "ucvtf"),
        ("fptosi", "double", "i32", "fcvtzs"),
        ("fptosi", "float", "i64", "fcvtzs"),
        ("fptoui", "float", "i32", "fcvtzu"),
        ("fptoui", "double", "i64", "fcvtzu"),
        ("fpext", "float", "double", "fcvt"),
        ("fptrunc", "double", "float", "fcvt"),
    ];
    for (op, from, to, wanted) in cases {
        let short = |ty: &str| ty.replace("double", "f64").replace("float", "f32");
        let (rounding, tail) = match op {
            "fpext" | "fptosi" | "fptoui" => ("", "metadata"),
            _ => (", metadata !\"round.dynamic\"", "metadata, metadata"),
        };
        let ir = format!(
            "{LAYOUT}define {to} @f({from} %x) strictfp {{\n  %r = call {to} @llvm.experimental.constrained.{op}.{}.{}({from} %x{rounding}, metadata !\"fpexcept.strict\") strictfp\n  ret {to} %r\n}}\ndeclare {to} @llvm.experimental.constrained.{op}.{}.{}({from}, {tail})\n",
            short(to),
            short(from),
            short(to),
            short(from),
        );
        let asm = driver::assembly(&ir, Platform::Linux, true).unwrap_or_else(|why| panic!("{op} {from}->{to}: {why}"));
        assert!(asm.contains(wanted), "{op} {from}->{to}: no {wanted}\n{asm}");
        let instructions = asm.lines().filter(|line| line.starts_with('\t')).count();
        assert_eq!(instructions, 2, "{op} {from}->{to}: the instruction and `ret` alone\n{asm}");
    }
}
