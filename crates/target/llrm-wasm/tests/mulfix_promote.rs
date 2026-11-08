mod common;

/// `llvm.{s,u}mul.fix[.sat]` and the narrow `{s,u}{add,sub}.sat`, `ushl.sat` at
/// i8 and i16 reached "promote result" and was refused ("Unsupported { op:
/// SMulFixSat, ty: I8 }"), so `-ffixed-point` code (tests/run/c/fixed_point.c)
/// did not compile on wasm32. The expected file is the pin's `llc -O2` output
/// for the same IR (`PromoteIntRes_MULFIX`).
#[test]
fn a_narrow_fixed_point_multiply_promotes_as_the_pin_does() {
    let (made, expected) = common::made_and_expected("mulfix_promote");
    assert_eq!(made, expected);
}
