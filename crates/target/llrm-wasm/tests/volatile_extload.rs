mod common;

/// A volatile load widened by a sign extension stayed `i32.load16_u` plus
/// `i32.extend16_s` (and `i32.load` plus `i64.extend_i32_s`): the extending-
/// load folds bailed on `volatile`, but the pin only does when the extending
/// load is not legal. +10 B in tests/run/c/overflow_builtins.c at O2. The
/// expected file is the pin's `llc -O2` output for the same IR.
#[test]
fn a_volatile_load_that_is_sign_extended_is_a_sign_extending_load() {
    let (made, expected) = common::made_and_expected("volatile_extload");
    assert_eq!(made, expected);
}
