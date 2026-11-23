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

/// `udivmod16` branches into `__udivhi3c` (`bne`), which the image scan read
/// as a call only: the speed divide's image came out 16 B smaller than the
/// compact one's (-16 for a 31 B growth), and the choice above never saw its
/// bytes. The growth is 125 B since the speed divide shifts the divisor up to
/// the dividend.
#[test]
fn an_image_includes_a_routine_the_speed_divide_branches_into() {
    use llrm_6502::mulimage::{image_bytes, image_of_texts};
    let names = vec!["f".to_owned()];
    let speed = image_of_texts(&names, &["  jsr __udivhi3\n".to_owned()]);
    let compact = image_of_texts(&names, &["  jsr __udivhi3c\n".to_owned()]);
    assert!(speed.contains("__udivhi3c"), "{speed:?}");
    assert_eq!(image_bytes(&speed) - image_bytes(&compact), 125, "{speed:?} against {compact:?}");
}

/// The compact word divide linked 55 B (an 8 B wrapper over a 47 B loop that
/// compared, then subtracted again, and tested a 17th bit that a 16-step loop
/// from a zero remainder never produces): k_tosudivax stood 6 B over sdcc's 64.
/// The loop now returns the quotient itself and subtracts once. The trial
/// compare keeps its borrow for the subtract that follows (2 B more than the
/// `sec`/`tay` form, 16 clocks a step on a quotient bit of 0 down to 12).
#[test]
fn the_compact_word_divide_image_is_44_bytes_and_the_remainder_one_52() {
    use llrm_6502::mulimage::{image_bytes, image_of_texts};
    let names = vec!["f".to_owned()];
    let divide = image_of_texts(&names, &["  jsr __udivhi3c\n".to_owned()]);
    let remainder = image_of_texts(&names, &["  jsr __umodhi3c\n".to_owned()]);
    assert!(image_bytes(&divide) <= 44, "{divide:?}: {} B", image_bytes(&divide));
    assert!(image_bytes(&remainder) <= 52, "{remainder:?}: {} B", image_bytes(&remainder));
}

/// The signed compact divide linked 109 B (2026-11-15): `neg16` twice in the
/// sign pair, the result reloaded and renegated. The pair is one loop over the
/// two words, and a result is negated in A:X: 95 B.
#[test]
fn the_compact_signed_word_divide_image_is_bounded() {
    use llrm_6502::mulimage::{image_bytes, image_of_texts};
    let names = vec!["f".to_owned()];
    let divide = image_of_texts(&names, &["  jsr __divhi3c\n".to_owned()]);
    let remainder = image_of_texts(&names, &["  jsr __modhi3c\n".to_owned()]);
    assert!(image_bytes(&divide) <= 95, "{divide:?}");
    assert!(image_bytes(&remainder) <= 98, "{remainder:?}: {} B", image_bytes(&remainder));
}

/// idiv_loop_os -Os linked the compact remainder (2.31M clocks against
/// llvm-mos's 1.07M) because the share rule priced the speed routine's bytes
/// against the module's, while the pin's own flow links 285 B for a signed
/// 16-bit remainder and 174 B for an unsigned one (`[pin.division16]`, llvm-mos
/// a40207a, linked image). Ours link 229 B and 169 B.
#[test]
fn the_speed_remainders_link_no_more_than_the_pin_does_for_the_same_kind() {
    use llrm_6502::mulimage::{image_bytes, image_of_texts};
    use llrm_6502::routines::within_pin_division16_image;
    let names = vec!["f".to_owned()];
    let signed = image_of_texts(&names, &["  jsr __modhi3\n".to_owned()]);
    let unsigned = image_of_texts(&names, &["  jsr __umodhi3\n".to_owned()]);
    assert_eq!(image_bytes(&signed), 229, "{signed:?}");
    assert_eq!(image_bytes(&unsigned), 169, "{unsigned:?}");
    assert!(within_pin_division16_image(&signed) && within_pin_division16_image(&unsigned));
}

/// The ceiling is the pin's for the kind: an unsigned image past 174 B (here
/// with a long division the pin does not link beside it) is refused, as the
/// signed image would be past 285 B.
#[test]
fn an_image_past_the_pins_for_its_kind_is_refused() {
    use llrm_6502::mulimage::{image_bytes, image_of_texts};
    use llrm_6502::routines::within_pin_division16_image;
    let names = vec!["f".to_owned()];
    let big = image_of_texts(&names, &["  jsr __umodhi3\n  jsr __udivsi3_fast\n".to_owned()]);
    assert!(image_bytes(&big) > 174 && !within_pin_division16_image(&big), "{big:?}: {} B", image_bytes(&big));
    let signed_big = image_of_texts(&names, &["  jsr __modhi3\n  jsr __divsi3_fast\n".to_owned()]);
    assert!(image_bytes(&signed_big) > 285 && !within_pin_division16_image(&signed_big), "{signed_big:?}");
}
