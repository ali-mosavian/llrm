//! A divide by a constant at `-Os`: the multiply-high sequence is seven
//! instructions and 28 bytes against `sdiv` and `msub`, which gcc's `-Os`
//! uses. The pin keeps the divide only at `minsize`; here the `-Os` trial
//! keeps it where the clocks the sequence saves are a smaller share than the
//! bytes it adds (DEVIATE SD535; `div_share_at_os.rs` for the loop). These run
//! one divide once.

use std::process::Command;

fn assembly(
    attrs: &str,
    body: &str,
) -> String {
    let level = if attrs.is_empty() { "-O2" } else { "-Os" };
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
        .args(["-S", level, "-o"])
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

#[test]
fn a_signed_divide_by_a_power_of_two_is_shifts_at_the_default() {
    let text = assembly("", "%r = sdiv i32 %a, 512");
    assert!(text.contains("sbfm") && !text.contains("sdiv"), "{text}");
}
