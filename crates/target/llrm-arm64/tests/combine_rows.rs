//! `mul` + `add`/`sub` stayed two instructions: matmul `-O2` ran 2330
//! instructions for the pin's 1793 (`MachineCombiner` makes `madd`/`msub`).

use std::process::Command;

const HEADER: &str = "target triple = \"arm64-apple-macosx\"\n\
target datalayout = \"e-m:o-p270:32:32-p271:32:32-p272:64:64-i64:64-i128:128-n32:64-S128-Fn32\"\n";

fn asm(
    level: &str,
    body: &str,
) -> String {
    let dir = tempfile::tempdir().unwrap();
    let input = dir.path().join("t.ll");
    std::fs::write(&input, format!("{HEADER}{body}")).unwrap();
    let output = dir.path().join("t.s");
    let said = Command::new(env!("CARGO_BIN_EXE_llrm-arm64-llc"))
        .args(["-S", level, "--isel=dag", "-o"])
        .arg(&output)
        .arg(&input)
        .output()
        .unwrap();
    assert!(said.status.success(), "{}", String::from_utf8_lossy(&said.stderr));
    std::fs::read_to_string(&output).unwrap()
}

fn function(
    ty: &str,
    expr: &str,
) -> String {
    format!("define {ty} @f({ty} %a, {ty} %b, {ty} %c) {{\n{expr}  ret {ty} %r\n}}\n")
}

#[test]
fn a_product_added_to_a_register_is_a_madd() {
    for (ty, reg) in [("i32", "w"), ("i64", "x")] {
        let text = asm("-O2", &function(ty, &format!("  %m = mul {ty} %a, %b\n  %r = add {ty} %m, %c\n")));
        assert!(text.contains(&format!(", {reg}2\n")) && text.contains("madd"), "{text}");
        assert!(!text.contains("zr\n"), "{text}");
    }
}

#[test]
fn a_register_less_a_product_is_an_msub() {
    let text = asm("-O2", &function("i32", "  %m = mul i32 %a, %b\n  %r = sub i32 %c, %m\n"));
    assert!(text.contains("msub w0, "), "{text}");
}

#[test]
fn a_product_less_a_register_negates_the_register_for_a_madd() {
    let text = asm("-O2", &function("i64", "  %m = mul i64 %a, %b\n  %r = sub i64 %m, %c\n"));
    assert!(text.contains("madd"), "{text}");
    assert!(text.contains("neg") || text.contains("sub x"), "{text}");
    assert!(!text.contains("zr\n"), "{text}");
}

#[test]
fn a_product_plus_a_small_constant_is_a_madd_with_the_constant_in_a_register() {
    let text =
        asm("-O2", "define i64 @f(i64 %a, i64 %b) {\n  %m = mul i64 %a, %b\n  %r = sub i64 %m, 128\n  ret i64 %r\n}\n");
    assert!(text.contains("madd"), "{text}");
    assert!(!text.contains("zr\n"), "{text}");
}

#[test]
fn a_product_read_twice_stays_a_mul() {
    let text = asm("-O2", &function("i32", "  %m = mul i32 %a, %b\n  %s = add i32 %m, %c\n  %r = xor i32 %s, %m\n"));
    assert!(text.contains("wzr"), "{text}");
}
