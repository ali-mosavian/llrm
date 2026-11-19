mod common;

/// `sshl.sat` at i8/i16 kept `select (x < 0), MIN, MAX` (the sign fold was
/// skipped for wasm's `i32` condition, and the new `sra` had a byte amount:
/// "CannotSelect Sra"), and an `xor` constant the next shift does not demand
/// was not shrunk. Expected: the pin's `llc -O2` output for the same IR.
#[test]
fn a_saturating_signed_shift_is_the_pins_code() {
    let (made, expected) = common::made_and_expected("shl_sat");
    assert_eq!(made, expected);
}
