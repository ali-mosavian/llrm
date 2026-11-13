//! A `<4 x float>` or `<2 x double>` operation: arm64 refused the function
//! ("vector with no register class") because the DAG made integer-lane
//! vectors only, so the loop vectorizer could not widen float arithmetic
//! (jpeg_single, stft_single). Hand IR, since no pass makes the vector yet.

use std::process::Command;

const HEAD: &str = "target triple = \"aarch64-unknown-linux-gnu\"\n\
target datalayout = \"e-m:e-p270:32:32-p271:32:32-p272:64:64-i8:8:32-i16:16:32-i64:64-i128:128-n32:64-S128-Fn32\"\n";

fn assembly(body: &str) -> String {
    let dir = tempfile::tempdir().unwrap();
    let (input, output) = (dir.path().join("t.ll"), dir.path().join("t.s"));
    std::fs::write(&input, format!("{HEAD}{body}")).unwrap();
    let said = Command::new(env!("CARGO_BIN_EXE_llrm-arm64-llc"))
        .args(["-S", "-O2", "-o"])
        .arg(&output)
        .arg(&input)
        .output()
        .unwrap();
    assert!(said.status.success(), "{}", String::from_utf8_lossy(&said.stderr));
    String::from_utf8(std::fs::read(&output).unwrap()).unwrap()
}

fn binary(
    ty: &str,
    op: &str,
) -> String {
    assembly(&format!(
        "define void @f(ptr %d, ptr %a, ptr %b) {{\n\
         \x20 %x = load {ty}, ptr %a, align 16\n\
         \x20 %y = load {ty}, ptr %b, align 16\n\
         \x20 %z = {op} {ty} %x, %y\n\
         \x20 store {ty} %z, ptr %d, align 16\n\
         \x20 ret void\n}}\n"
    ))
}

/// The instruction the function is, with its register numbers dropped.
fn has(
    text: &str,
    mnemonic: &str,
    suffix: &str,
) -> bool {
    text.lines().any(|line| {
        let line = line.trim();
        line.strip_prefix(mnemonic)
            .is_some_and(|rest| rest.starts_with(' ') && rest.matches(&format!(".{suffix}")).count() == 3)
    })
}

#[test]
fn float_vector_arithmetic_is_one_neon_instruction() {
    for (op, mnemonic) in [("fadd", "fadd"), ("fsub", "fsub"), ("fmul", "fmul"), ("fdiv", "fdiv")] {
        let single = binary("<4 x float>", op);
        assert!(has(&single, mnemonic, "4s"), "{op} <4 x float>:\n{single}");
        let double = binary("<2 x double>", op);
        assert!(has(&double, mnemonic, "2d"), "{op} <2 x double>:\n{double}");
    }
}

#[test]
fn a_float_vector_negation_is_fneg() {
    let text = assembly(
        "define void @f(ptr %d, ptr %a) {\n\
         \x20 %x = load <4 x float>, ptr %a, align 16\n\
         \x20 %z = fneg <4 x float> %x\n\
         \x20 store <4 x float> %z, ptr %d, align 16\n\
         \x20 ret void\n}\n",
    );
    assert!(text.lines().any(|l| l.trim().starts_with("fneg ") && l.contains(".4s")), "{text}");
}

/// `llvm.fmuladd` of float vectors selected `FMLAv4f32`, which had no
/// instruction ("machine opcode FMLAv4f32 has no arm64 instruction yet"): the
/// loops of jpeg_single's IDCT could not be widened.
#[test]
fn a_vector_fmuladd_is_one_fmla() {
    for (ty, suffix, name) in [("<4 x float>", "4s", "v4f32"), ("<2 x double>", "2d", "v2f64")] {
        let text = assembly(&format!(
            "declare {ty} @llvm.fmuladd.{name}({ty}, {ty}, {ty})\n\
             define void @f(ptr %d, ptr %a, ptr %b, ptr %c) {{\n\
             \x20 %x = load {ty}, ptr %a, align 16\n\
             \x20 %y = load {ty}, ptr %b, align 16\n\
             \x20 %z = load {ty}, ptr %c, align 16\n\
             \x20 %r = call {ty} @llvm.fmuladd.{name}({ty} %x, {ty} %y, {ty} %z)\n\
             \x20 store {ty} %r, ptr %d, align 16\n\
             \x20 ret void\n}}\n"
        ));
        assert!(has(&text, "fmla", suffix), "fmuladd {ty}:\n{text}");
    }
}

/// A float vector constant (the splat of 2.5) had no lowering ("a vector
/// constant lane that is not an integer"), and the splat of a lane no `dup`.
#[test]
fn a_float_vector_constant_splat_is_a_dup() {
    let text = assembly(
        "define void @f(ptr %d, ptr %a) {\n\
         \x20 %x = load <4 x float>, ptr %a, align 16\n\
         \x20 %z = fadd <4 x float> %x, <float 2.5, float 2.5, float 2.5, float 2.5>\n\
         \x20 store <4 x float> %z, ptr %d, align 16\n\
         \x20 ret void\n}\n",
    );
    assert!(has(&text, "fadd", "4s"), "{text}");
    assert!(text.lines().any(|l| l.trim().starts_with("dup v") && l.contains(".4s")), "{text}");
}

/// `sitofp`/`uitofp`/`fptosi`/`fptoui` of vectors of one lane width: the
/// verifier said "cannot take <4 x i32> to <4 x float>", and then no
/// instruction existed for `SCVTFv4f32`.
#[test]
fn vector_int_float_conversions_are_one_neon_instruction() {
    for (op, from, to, mnemonic, suffix) in [
        ("sitofp", "<4 x i32>", "<4 x float>", "scvtf", "4s"),
        ("uitofp", "<4 x i32>", "<4 x float>", "ucvtf", "4s"),
        ("fptosi", "<4 x float>", "<4 x i32>", "fcvtzs", "4s"),
        ("fptoui", "<4 x float>", "<4 x i32>", "fcvtzu", "4s"),
        ("sitofp", "<2 x i64>", "<2 x double>", "scvtf", "2d"),
        ("fptosi", "<2 x double>", "<2 x i64>", "fcvtzs", "2d"),
    ] {
        let text = assembly(&format!(
            "define void @f(ptr %d, ptr %a) {{\n\
             \x20 %x = load {from}, ptr %a, align 16\n\
             \x20 %z = {op} {from} %x to {to}\n\
             \x20 store {to} %z, ptr %d, align 16\n\
             \x20 ret void\n}}\n"
        ));
        let one = text.lines().any(|line| {
            line.trim().strip_prefix(mnemonic).is_some_and(|rest| rest.matches(&format!(".{suffix}")).count() == 2)
        });
        assert!(one, "{op} {from}:\n{text}");
    }
}
