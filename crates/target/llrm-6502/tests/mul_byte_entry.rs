//! `x * 11` on a 32-bit value called `__mulsi3` with the multiplier's upper
//! three bytes cleared into rc5..rc7 first (12 B a site): the call reads only
//! rc4 now, through the byte entries.

mod common;

use common::pipeline::datalayout;
use llrm_6502::driver::{Level, Request, assembly};

fn times_eleven(level: Level) -> String {
    let ir = format!(
        "target datalayout = \"{}\"\ndefine i32 @f(i32 %x, i32 %y) {{\n  %a = mul i32 %x, 11\n  %b = mul i32 %y, 11\n  %c = xor i32 %a, %b\n  ret i32 %c\n}}\n",
        datalayout()
    );
    assembly(&ir, &Request::new(level)).unwrap_or_else(|e| panic!("{e}"))
}

#[test]
fn a_long_multiply_by_a_byte_calls_the_byte_entry_and_clears_no_upper_bytes() {
    for level in [Level::Os] {
        let text = times_eleven(level);
        let calls = text.matches("__mulsi3_byte\n").count() + text.matches("__mulsi3_fast_byte\n").count();
        assert!(calls >= 1, "{level:?}: no byte entry:\n{text}");
        for upper in ["sta rc5", "sta rc6", "sta rc7"] {
            assert!(!text.contains(upper), "{level:?}: {upper} still set up:\n{text}");
        }
    }
}
