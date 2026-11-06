//! The carry of a long add or subtract stays in the flag.

use std::process::Command;

fn assembly(
    name: &str,
    ir: &str,
) -> String {
    let dir = std::env::temp_dir().join(format!("llrm-carry-z80-{name}"));
    std::fs::create_dir_all(&dir).unwrap();
    let source = dir.join("g.ll");
    std::fs::write(&source, format!("target datalayout = \"e-p:16:8-i16:8-i32:8-i64:8-f32:8-f64:8-a:8-n8:16\"\n{ir}"))
        .unwrap();
    let out = dir.join("g.s");
    let done = Command::new(env!("CARGO_BIN_EXE_llrm-z80-llc"))
        .args(["-S", "-O2", "-o", out.to_str().unwrap(), source.to_str().unwrap()])
        .output()
        .expect("the driver runs");
    assert_eq!(done.status.code(), Some(0), "{}", String::from_utf8_lossy(&done.stderr));
    std::fs::read_to_string(out).unwrap()
}

/// `a + b` on longs rebuilt the carry by a compare, `jr c` and a byte added to
/// the high word (11 instructions and 3 branches beyond the two adds); the
/// high word is `adc hl, rr`.
#[test]
fn a_long_add_carries_in_the_flag() {
    let text = assembly("add", "define i32 @g(i32 %a, i32 %b) {\n  %r = add i32 %a, %b\n  ret i32 %r\n}\n");
    assert!(text.contains("adc hl, ") && !text.contains("jr c"), "{text}");
}

/// The same for the borrow: `sbc hl, rr` after `sbc hl, rr`.
#[test]
fn a_long_subtract_borrows_in_the_flag() {
    let text = assembly("sub", "define i32 @g(i32 %a, i32 %b) {\n  %r = sub i32 %a, %b\n  ret i32 %r\n}\n");
    assert!(text.matches("sbc hl, ").count() == 2 && !text.contains("jr c"), "{text}");
}

/// A long grown by a widened word: the high word has no addend of its own and
/// the carry was still rebuilt by a compare (mem_length_boundaries_m32 -O2: the
/// 32-bit sum of a loop, 11 instructions a trip beyond `adc hl, rr`).
#[test]
fn a_long_grown_by_a_word_carries_in_the_flag() {
    let text = assembly(
        "widened",
        "define i32 @g(i32 %a, i16 %b) {\n  %e = zext i16 %b to i32\n  %r = add i32 %a, %e\n  ret i32 %r\n}\n",
    );
    assert!(text.contains("adc hl, ") && !text.contains("jr c"), "{text}");
}
