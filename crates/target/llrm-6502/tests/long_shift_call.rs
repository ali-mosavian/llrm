//! A long shift by a constant is a call of the routine at -Os when the shift in
//! line is longer: `call_keeps` -Os carried twelve inline shift loops (13 to 20
//! bytes each beside a 5-byte `ldy; jsr`).

mod common;

use common::pipeline::datalayout;
use llrm_6502::driver::{Level, Request, assembly};

fn asm(
    level: Level,
    attributes: &str,
    shift: &str,
    count: u32,
) -> String {
    let ir = format!(
        "target datalayout = \"{}\"\ndefine i32 @f(i32 %x) {attributes} nounwind {{\n  %r = {shift} i32 %x, {count}\n  ret i32 %r\n}}\n",
        datalayout()
    );
    assembly(&ir, &Request::new(level)).unwrap_or_else(|e| panic!("{e}"))
}

#[test]
fn a_long_shift_by_three_bits_is_a_call_at_size() {
    for (shift, routine) in [("shl", "__ashlsi3"), ("lshr", "__lshrsi3"), ("ashr", "__ashrsi3")] {
        let text = asm(Level::Os, "minsize", shift, 3);
        assert!(text.contains(routine), "{shift}\n{text}");
    }
}

/// A shift of whole bytes, or of one bit, stays in line; so does any shift at
/// speed.
#[test]
fn a_cheap_shift_or_a_shift_at_speed_stays_in_line() {
    for count in [1, 8, 16, 24] {
        let text = asm(Level::Os, "minsize", "shl", count);
        assert!(!text.contains("__ashlsi3"), "{count}\n{text}");
    }
    let text = asm(Level::O2, "", "shl", 3);
    assert!(!text.contains("__ashlsi3"), "{text}");
}
