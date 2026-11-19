mod common;

/// A rotate of a byte or halfword had no `PromoteIntRes_Rotate`: "Unsupported { op: Rotl, ty: I8, what: promote result }" at
/// every level, so tests/run/c/rotate_small.c -Os did not build. The expected file is the pin llc's `-O2` output.
#[test]
fn a_rotate_of_a_small_integer_is_the_pins_code() {
    let (made, expected) = common::made_and_expected("rotate_small");
    assert_eq!(made, expected);
}
