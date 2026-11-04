//! Sign extension and arithmetic right shifts by whole bytes are byte moves,
//! not a shift a bit.

use std::process::Command;

fn assembly(
    name: &str,
    ir: &str,
) -> String {
    let dir = std::env::temp_dir().join(format!("llrm-sign-extend-z80-{name}"));
    std::fs::create_dir_all(&dir).unwrap();
    let source = dir.join("g.ll");
    std::fs::write(
        &source,
        format!("target datalayout = \"e-p:16:8-i16:8-i32:8-i64:8-f32:8-f64:8-a:8-n8:16\"\n{ir}"),
    )
    .unwrap();
    let out = dir.join("g.s");
    let done = Command::new(env!("CARGO_BIN_EXE_llrm-z80-llc"))
        .args(["-S", "-O2", "-o", out.to_str().unwrap(), source.to_str().unwrap()])
        .output()
        .expect("the driver runs");
    assert_eq!(done.status.code(), Some(0), "{}", String::from_utf8_lossy(&done.stderr));
    std::fs::read_to_string(out).unwrap()
}

/// `sext i8 to i32` of a loaded byte was `ld d,(hl); ld e,#0` and 8 + 16 `sra;
/// rr` pairs (rep_count's sum loop: 870 B against SDCC's 273).
#[test]
fn a_loaded_byte_is_sign_extended_without_shifts() {
    let text = assembly(
        "load",
        "define i32 @g(ptr %p) {\n  %b = load i8, ptr %p\n  %r = sext i8 %b to i32\n  ret i32 %r\n}\n",
    );
    assert!(!text.contains("sra"), "{text}");
    assert!(text.lines().count() < 20, "{text}");
}

/// `ashr i16 %x, 15` was fifteen `sra; rr` pairs.
#[test]
fn a_word_shifted_right_by_a_byte_or_more_keeps_its_sign_by_the_high_byte() {
    let text = assembly("word", "define i16 @g(i16 %x) {\n  %r = ashr i16 %x, 15\n  ret i16 %r\n}\n");
    assert!(!text.contains("sra"), "{text}");
    assert!(text.lines().count() < 14, "{text}");
}

/// `ashr i8 %x, 7` was seven `sra a`: the sign alone is `add a, a; sbc a, a`.
#[test]
fn the_sign_of_a_byte_is_the_carry_of_its_top_bit() {
    let text = assembly("byte", "define i8 @g(i8 %x) {\n  %r = ashr i8 %x, 7\n  ret i8 %r\n}\n");
    assert!(text.contains("add a, a") && text.contains("sbc a, a") && !text.contains("sra"), "{text}");
}
