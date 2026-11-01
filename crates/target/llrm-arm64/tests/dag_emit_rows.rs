//! Machine opcodes the DAG path emitted as "has no arm64 instruction yet":
//! 16 of the corpus's bench kernels (queens `place`, ...) were refused by
//! `--isel=dag` and fell to the hand selector.

use std::process::Command;

const HEADER: &str = "target triple = \"arm64-apple-macosx\"\n\
target datalayout = \"e-m:o-p270:32:32-p271:32:32-p272:64:64-i64:64-i128:128-n32:64-S128-Fn32\"\n";

fn dag(body: &str) -> Result<String, String> {
    let dir = tempfile::tempdir().unwrap();
    let input = dir.path().join("t.ll");
    std::fs::write(&input, format!("{HEADER}{body}")).unwrap();
    let output = dir.path().join("t.s");
    let said = Command::new(env!("CARGO_BIN_EXE_llrm-arm64-llc"))
        .args(["-S", "--no-opt", "--isel=dag", "-o"])
        .arg(&output)
        .arg(&input)
        .output()
        .unwrap();
    if said.status.success() {
        Ok(std::fs::read_to_string(&output).unwrap())
    } else {
        Err(String::from_utf8_lossy(&said.stderr).into_owned())
    }
}

/// `add x, y, sext(z)` is the pin's `ADDXrx` (`add x0, x0, w1, sxtw`).
#[test]
fn an_add_of_a_sign_extended_word_is_the_extended_register_add() {
    let asm =
        dag("define i64 @f(i64 %a, i32 %b) {\n  %e = sext i32 %b to i64\n  %r = add i64 %a, %e\n  ret i64 %r\n}\n")
            .unwrap_or_else(|why| panic!("{why}"));
    assert!(asm.contains("add x0, x0, w1, sxtw"), "{asm}");
}

/// The high half of a 64x64 multiply is `SMULHrr`.
#[test]
fn the_high_half_of_a_signed_multiply_is_smulh() {
    let asm = dag("define i64 @f(i64 %a, i64 %b) {\n  %x = sext i64 %a to i128\n  %y = sext i64 %b to i128\n  %m = mul i128 %x, %y\n  %h = lshr i128 %m, 64\n  %r = trunc i128 %h to i64\n  ret i64 %r\n}\n")
        .unwrap_or_else(|why| panic!("{why}"));
    assert!(asm.contains("smulh x0, x0, x1"), "{asm}");
}
