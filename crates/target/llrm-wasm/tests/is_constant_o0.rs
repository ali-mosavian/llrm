mod common;

/// At -O0 no mid-end ran `LowerConstantIntrinsics`, and the backend refused
/// `llvm.is.constant` ("intrinsic call llvm.is.constant.i32":
/// tests/run/c/builtins_table.c -O0 did not build). The pin lowers it before
/// selection at every level: 1 for a constant argument, 0 for a value. The
/// expected file is the pin llc's `-O0` output.
#[test]
fn is_constant_is_lowered_at_o0() {
    let (made, expected) = common::made_and_expected_at("-O0", "is_constant_o0");
    assert_eq!(made, expected);
}
