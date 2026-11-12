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
