mod common;

/// `umulo`/`smulo` had no `visitMULO` and the wide product was inline, not
/// `__multi3`: an i16 product that cannot overflow in i32 went through an
/// i64 multiply and an `or` of zero (+118 B in tests/run/c/overflow_builtins.c
/// at Os), an i64 one through twelve inline multiplies, and `store (trunc x)`
/// was `i32.wrap_i64; i32.store`, not `i64.store32`. The expected file is the
/// pin's `llc -O2` output for the same IR.
#[test]
fn a_multiply_with_overflow_is_the_pins_code() {
    let (made, expected) = common::made_and_expected("mulo");
    assert_eq!(made, expected);
}
