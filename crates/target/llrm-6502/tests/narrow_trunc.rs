//! A word op whose result is truncated to a byte runs on bytes.

mod common;

use common::pipeline::datalayout;
use llrm_6502::driver::{Level, Request, assembly};

/// `p - s` returned as a byte (`schr`) subtracted the high bytes as well and
/// dropped the result: 47 B at -O2, two `sbc`. Only the low byte is demanded,
/// so the subtract runs on bytes.
#[test]
fn a_truncated_word_difference_is_a_byte_subtract() {
    let ir = "define i8 @diff(ptr %p, ptr %s) nounwind {\n  %a = ptrtoint ptr %p to i16\n  %b = ptrtoint ptr %s to i16\n  %d = sub i16 %a, %b\n  %t = trunc i16 %d to i8\n  ret i8 %t\n}\n";
    let ir = format!("target datalayout = \"{}\"\n{ir}", datalayout());
    let text = assembly(&ir, &Request::new(Level::O2)).unwrap_or_else(|e| panic!("{e}"));
    let from = text.find("diff:").unwrap_or_else(|| panic!("{text}"));
    let body = text[from..].split(".segment").next().unwrap();
    assert_eq!(body.matches("sbc").count(), 1, "{body}");
}

/// The lane a truncated word leaves unread was still moved (`pha; ldx rc3;
/// pla` beside the byte subtract, three instructions that nothing read).
#[test]
fn a_lane_nothing_reads_is_not_moved() {
    let ir = "define i8 @diff(ptr %p, ptr %s) nounwind {\n  %a = ptrtoint ptr %p to i16\n  %b = ptrtoint ptr %s to i16\n  %d = sub i16 %a, %b\n  %t = trunc i16 %d to i8\n  ret i8 %t\n}\n";
    let ir = format!("target datalayout = \"{}\"\n{ir}", datalayout());
    let text = assembly(&ir, &Request::new(Level::O2)).unwrap_or_else(|e| panic!("{e}"));
    let from = text.find("diff:").unwrap_or_else(|| panic!("{text}"));
    let body = text[from..].split(".segment").next().unwrap();
    assert!(!body.contains("pha") && !body.contains("ldx"), "{body}");
}

/// `sum + zext(select(b, 2, 0) | zext(bool))` (`logical_value`) widened the byte-sized
/// `or` to a word first, then added the high word with `adc rc7`: 103 B at -O2 against
/// 85. The high bits of the `or` are known zero, so the `or` runs on bytes and the
/// three upper bytes add `#0`.
#[test]
fn a_value_known_to_fit_a_byte_adds_zero_to_the_upper_bytes() {
    let ir = "define i32 @f(i32 %sum, i16 %a, i16 %b) nounwind {\n  %c1 = icmp ne i16 %a, 0\n  %c2 = icmp ne i16 %b, 0\n  %n = xor i1 %c2, true\n  %an = and i1 %c1, %n\n  %z = zext i1 %an to i16\n  %s = select i1 %c2, i16 2, i16 0\n  %o = or disjoint i16 %s, %z\n  %w = zext nneg i16 %o to i32\n  %r = add nsw i32 %sum, %w\n  ret i32 %r\n}\n";
    let ir = format!("target datalayout = \"{}\"\n{ir}", datalayout());
    let text = assembly(&ir, &Request::new(Level::O2)).unwrap_or_else(|e| panic!("{e}"));
    let from = text.find("f:").unwrap_or_else(|| panic!("{text}"));
    let body = text[from..].split(".segment").next().unwrap();
    assert_eq!(body.matches("adc #0").count(), 3, "{body}");
}

/// `zext(and(x, 255))` compiled forever: sizing an `and` by its known-zero bits turned
/// it into `zext(and8(trunc x, 255))`, which folds back into the `and` and was narrowed
/// again (`compound_assignment_types` at -O1 never finished).
#[test]
fn a_masked_word_widened_to_a_long_compiles() {
    let ir = "define i32 @f(i16 %t, i32 %a) nounwind {\n  %m = and i16 %t, 255\n  %z = zext nneg i16 %m to i32\n  %r = add nsw i32 %a, %z\n  ret i32 %r\n}\n";
    let ir = format!("target datalayout = \"{}\"\n{ir}", datalayout());
    let text = assembly(&ir, &Request::new(Level::O1)).unwrap_or_else(|e| panic!("{e}"));
    assert!(text.contains("f:"), "{text}");
}
