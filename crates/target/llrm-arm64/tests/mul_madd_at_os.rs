//! A multiply by a constant that an add or subtract reads is a `madd` at
//! `optsize` (DEVIATE MUL-MADD-OS). Before, `x * 7 + y` was `sub` and
//! `add .. lsl` (histo -Os: 11 instructions a trip against gcc's 9).

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
             define i32 @f(i32 %a, i32 %b) {attrs} {{\n {body}\n ret i32 %r\n}}\n"
        ),
    )
    .unwrap();
    let said = Command::new(env!("CARGO_BIN_EXE_llrm-arm64-llc"))
        .args(["-S", "-O2", "-o"])
        .arg(&output)
        .arg(&input)
        .output()
        .unwrap();
    assert!(said.status.success(), "{}", String::from_utf8_lossy(&said.stderr));
    String::from_utf8(std::fs::read(&output).unwrap()).unwrap()
}

const BODY: &str = "%m = mul i32 %a, 7\n %r = add i32 %m, %b";

#[test]
fn a_multiply_read_by_an_add_is_a_madd_at_optsize() {
    let text = assembly("optsize", BODY);
    assert!(text.contains("madd") && !text.contains("lsl #3"), "{text}");
}

#[test]
fn it_is_shift_and_subtract_at_the_default() {
    let text = assembly("", BODY);
    assert!(text.contains("lsl #3") && !text.contains("madd"), "{text}");
}
