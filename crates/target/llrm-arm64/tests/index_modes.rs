//! Shifts and extensions folded into an indexed address.

use llrm_arm64::driver;
use llrm_arm64::machine::Platform;

fn body(name: &str) -> String {
    let asm = driver::assembly(include_str!("data/index_modes.ll"), Platform::Darwin, true).expect("lowers");
    let start = asm.find(&format!("_{name}:")).expect("the function is there");
    let rest = &asm[start..];
    rest[..rest[1..].find("\n_").map_or(rest.len(), |end| end + 1)].to_owned()
}

/// `p + (i << 1)` stored as a halfword was `ubfm` (a shift) and `strh [x0, x9]`; clang indexes by `x1, lsl #1`.
#[test]
fn a_shifted_index_is_the_addressing_mode() {
    let asm = body("shifted");
    assert!(asm.contains("strh w2, [x0, x1, lsl #1]"), "{asm}");
}

/// A sign- or zero-extended 32-bit index was widened by `sbfm`/`mov`, then used; clang extends in the access.
#[test]
fn an_extended_index_is_the_addressing_mode() {
    assert!(body("signed").contains("[x0, w1, sxtw #2]"));
    assert!(body("unsigned").contains("[x0, w1, uxtw #2]"));
}

/// The widened value is read again elsewhere: it stays a value, and the access uses it as it is.
#[test]
fn an_index_read_twice_is_still_extended_once() {
    let asm = body("index_read_twice");
    assert_eq!(asm.matches("sbfm").count(), 1, "{asm}");
}

/// A shift of three on 8-byte elements would be a shift of six: not an addressing mode.
#[test]
fn a_shift_past_the_addressing_mode_stays_a_shift() {
    let asm = body("too_wide");
    assert!(asm.contains("ubfm") || asm.contains("lsl"), "{asm}");
}

/// A loaded `i32` that is only sign-extended as an index was both a sign-extending load and an indexed-address
/// extension: two folds of one `sext`, and the lowering failed ("value is read before it is selected") on
/// 42 programs at -O0; and the same with an element size that is no addressing mode, where only the load folds.
#[test]
fn a_loaded_index_lowers_at_every_level() {
    for level in [driver::Level::O0, driver::Level::O2] {
        let asm = driver::assembly(include_str!("data/index_modes.ll"), Platform::Darwin, level).expect("lowers");
        assert!(asm.contains("_loaded_index:") && asm.contains("_loaded_index_into_a_record_array:"), "{asm}");
    }
}
