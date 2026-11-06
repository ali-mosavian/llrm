//! A word shifted left by two or more outside `hl` doubles through `hl`.

use std::process::Command;

fn assembly(
    name: &str,
    ir: &str,
) -> String {
    let dir = std::env::temp_dir().join(format!("llrm-shift-hl-z80-{name}"));
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

/// `b << 3` in `de` was `sla e; rl d` three times, 12 B / 48 T; `ex de, hl; add hl, hl` x3 `; ex de, hl` is 5 B / 41 T
/// (huffman_decode's row stride, 22 B for x18).
#[test]
fn a_word_in_de_shifted_left_doubles_through_hl() {
    let text = assembly("de", "define i16 @g(i16 %a, i16 %b) {\n  %r = shl i16 %b, 3\n  ret i16 %r\n}\n");
    assert!(!text.contains("sla e"), "{text}");
    assert_eq!(text.matches("add hl, hl").count(), 3, "{text}");
}

/// The row stride of huffman_decode's tables, `x * 18`, was 22 B (`sla e; rl d` x4 beside `add hl, hl`).
#[test]
fn a_multiply_by_eighteen_has_no_byte_pair_shifts() {
    let text = assembly("x18", "define i16 @g(i16 %a, i16 %b) {\n  %r = mul i16 %b, 18\n  ret i16 %r\n}\n");
    assert!(!text.contains("sla e") && !text.contains("sla c"), "{text}");
}
