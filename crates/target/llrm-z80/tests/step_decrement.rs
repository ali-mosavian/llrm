//! A long decremented by one borrows into its high word by a zero test and a
//! `jr` over `dec rr`.

use llrm_z80::compile::text;
use llrm_z80::driver::{Level, Request};

const IR: &str = "target datalayout = \"e-p:16:8-i16:8-i32:8-i64:8-f32:8-f64:8-a:8-n8:16-S8\"
define i32 @dec(i32 %x) {
  %r = sub i32 %x, 1
  ret i32 %r
}
";

/// long_step_by_one `dec` took the borrow in `hl`: `ld c, l; ld b, h; ...; ex
/// de, hl; jr nz; dec hl; ex de, hl; dec bc; ld l, c; ld h, b` (13 B, SDCC's
/// whole function 7). The high word is in `de` and steps there.
#[test]
fn a_long_decrement_steps_its_high_word_in_de() {
    for level in [Level::O1, Level::Os, Level::Oz] {
        let out = text(IR, "dec", 0, &Request::new(level)).expect("compiles");
        assert!(out.contains("dec de") && out.contains("dec hl"), "{level:?}:\n{out}");
        assert!(!out.contains("ex de, hl") && !out.contains("ld c, l"), "{level:?}:\n{out}");
    }
}
