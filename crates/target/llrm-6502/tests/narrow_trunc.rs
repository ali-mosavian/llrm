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
