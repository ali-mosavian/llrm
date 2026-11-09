//! `__mulhi3` shifts its second operand right and stops when no bit is left,
//! so the operand with fewer significant bits goes second. A byte widened to
//! a word went first when the source wrote it first: 16 passes, not 8
//! (`loop_versioning` `a[i * s]`: +24,748 clocks once `s` arrived in A:X).

mod common;

use common::pipeline::datalayout;
use llrm_6502::driver::{Level, Request, assembly};

fn body(
    level: Level,
    first: &str,
    second: &str,
) -> String {
    let ir = format!(
        "target datalayout = \"{}\"\ndefine i16 @f(i16 %b, i16 %c) {{\n  %x = and i16 %b, 255\n  %m = mul i16 {first}, {second}\n  ret i16 %m\n}}\n",
        datalayout()
    );
    let text = assembly(&ir, &Request::new(level)).unwrap_or_else(|e| panic!("{e}"));
    let from = text.find("f:").unwrap_or_else(|| panic!("no f in\n{text}"));
    text[from..].split(".segment").next().unwrap().to_owned()
}

#[test]
fn a_masked_byte_is_the_multiplier_whichever_way_the_source_writes_it() {
    let (masked_first, masked_second) = (body(Level::O2, "%x", "%c"), body(Level::O2, "%c", "%x"));
    assert_eq!(masked_first, masked_second, "the order of the source decides:\n{masked_first}\n{masked_second}");
}

/// At -Os the byte count rules, and the source order that needs no copies
/// stays.
#[test]
fn at_os_the_order_that_needs_no_copies_stays() {
    let text = body(Level::Os, "%x", "%c");
    assert_eq!(text, "f:\n  ldx #0\n  jmp __mulhi3\n");
}
