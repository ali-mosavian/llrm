//! A divide by a constant at `-Os`: the multiply-high sequence is seven
//! instructions and 28 bytes against `sdiv` and `msub`, which gcc's `-Os`
//! uses. The pin keeps the divide only at `minsize`; here `optsize` does too
//! (DEVIATE SD534). Before, `recsum`, `recpow`, `recmany`, `switch`, `frames`
//! and `huge` at -Os were 25-60% above gcc on bytes.

use std::process::Command;

fn assembly(
    attrs: &str,
    body: &str,
) -> String {
    let dir = tempfile::tempdir().unwrap();
    let (input, output) = (dir.path().join("t.ll"), dir.path().join("t.s"));
    std::fs::write(
        &input,
        format!(
            "target triple = \"aarch64-unknown-linux-gnu\"\ntarget datalayout = \"e-m:e-p270:32:32-p271:32:32-p272:64:64-i8:8:32-i16:16:32-i64:64-i128:128-n32:64-S128-Fn32\"\n\
             define i32 @f(i32 %a) {attrs} {{\n {body}\n ret i32 %r\n}}\n"
        ),
    )
    .unwrap();
    let said = Command::new(env!("CARGO_BIN_EXE_llrm-arm64-llc"))
        .args(["-S", "-O2", "--isel=dag", "-o"])
        .arg(&output)
        .arg(&input)
        .output()
        .unwrap();
    assert!(said.status.success(), "{}", String::from_utf8_lossy(&said.stderr));
    String::from_utf8(std::fs::read(&output).unwrap()).unwrap()
}

#[test]
fn a_remainder_by_a_constant_is_sdiv_and_msub_at_optsize() {
    let text = assembly("optsize", "%r = srem i32 %a, 11");
    assert!(text.contains("sdiv") && text.contains("msub"), "{text}");
    assert!(!text.contains("smaddl"), "{text}");
}

#[test]
fn an_unsigned_divide_by_a_constant_is_udiv_at_optsize() {
    let text = assembly("optsize", "%r = udiv i32 %a, 7");
    assert!(text.contains("udiv"), "{text}");
}

#[test]
fn the_multiply_high_sequence_stays_at_the_default() {
    let text = assembly("", "%r = srem i32 %a, 11");
    assert!(text.contains("smaddl") && !text.contains("sdiv"), "{text}");
}
