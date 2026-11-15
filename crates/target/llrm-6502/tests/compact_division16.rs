//! At -O2 the 16-bit divisions were the speed routine, 31 B over the compact
//! one, whatever the module ran: k_tosudivax linked 101 B against the bytes bar
//! of 64 (2026-11-14, bytes first on this target at every level). A module
//! whose divisions are the clocks of its run takes the compact one: the speed
//! routine saves 215 clocks a call on average, and costs 31 B.

mod common;

use common::pipeline::datalayout;
use llrm_6502::driver::{Level, Request, assembly};

fn text(body: &str) -> String {
    let ir = format!("target datalayout = \"{}\"\n{body}", datalayout());
    assembly(&ir, &Request::new(Level::O2)).unwrap_or_else(|e| panic!("{e}"))
}

#[test]
fn a_cold_division_calls_the_compact_routine_at_speed() {
    let text = text("define i16 @q(i16 %a, i16 %b) {\n  %q = udiv i16 %a, %b\n  ret i16 %q\n}\n");
    assert!(text.contains("__udivhi3c") && !text.contains("__udivhi3\n"), "the compact routine at -O2:\n{text}");
}

/// `udivmod16` branches into `udivmod16c` (`bne`), which the image scan read
/// as a call only: the speed divide's image came out 16 B smaller than the
/// compact one's (-16 for a 31 B growth), and the choice above never saw its
/// bytes.
#[test]
fn an_image_includes_a_routine_the_speed_divide_branches_into() {
    use llrm_6502::mulimage::{image_bytes, image_of_texts};
    let names = vec!["f".to_owned()];
    let speed = image_of_texts(&names, &["  jsr __udivhi3\n".to_owned()]);
    let compact = image_of_texts(&names, &["  jsr __udivhi3c\n".to_owned()]);
    assert!(speed.contains("udivmod16c"), "{speed:?}");
    assert_eq!(image_bytes(&speed) - image_bytes(&compact), 31, "{speed:?} against {compact:?}");
}
