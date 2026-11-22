//! QuickrBasic -O3 `using.c` and `stmts.c` were refused (`CannotSelect
//! ExtractElement of V2I32`): the SLP vectorizer's `<2 x i32>` udiv/urem of
//! the clock, a lane taken out. Only the 128-bit vectors and lane 0 of a
//! 32-bit one were selectable; `LowerEXTRACT_VECTOR_ELT` widens a 64-bit
//! vector to 128 bits and takes the lane there.

use std::process::Command;

fn lane(
    ty: &str,
    lanes: u32,
    op: &str,
    index: u32,
) -> String {
    let dir = tempfile::tempdir().unwrap();
    let (input, output) = (dir.path().join("t.ll"), dir.path().join("t.s"));
    std::fs::write(
        &input,
        format!(
            "target triple = \"aarch64-unknown-linux-gnu\"
target datalayout = \"e-m:e-p270:32:32-p271:32:32-p272:64:64-i8:8:32-i16:16:32-i64:64-i128:128-n32:64-S128-Fn32\"
define {ty} @f({ty} %x) nounwind {{
  %a = insertelement <{lanes} x {ty}> poison, {ty} %x, i64 0
  %b = shufflevector <{lanes} x {ty}> %a, <{lanes} x {ty}> poison, <{lanes} x i32> zeroinitializer
  %c = {op} <{lanes} x {ty}> %b, %b
  %e = extractelement <{lanes} x {ty}> %c, i64 {index}
  ret {ty} %e
}}
"
        ),
    )
    .unwrap();
    let said = Command::new(env!("CARGO_BIN_EXE_llrm-arm64-llc"))
        .args(["-S", "-Os", "-o"])
        .arg(&output)
        .arg(&input)
        .output()
        .unwrap();
    assert!(said.status.success(), "{ty} x {lanes} lane {index}: {}", String::from_utf8_lossy(&said.stderr));
    std::fs::read_to_string(&output).unwrap()
}

#[test]
fn a_lane_of_a_64_bit_integer_vector_is_a_umov() {
    for (ty, lanes, index) in [("i8", 8, 3), ("i16", 4, 2), ("i32", 2, 1)] {
        let asm = lane(ty, lanes, "add", index);
        assert!(asm.contains("umov"), "{ty} x {lanes} lane {index}: {asm}");
    }
}

#[test]
fn a_lane_of_a_64_bit_float_vector_is_selected() {
    let asm = lane("float", 2, "fadd", 1);
    assert!(asm.contains("v16.s[1]") || asm.contains(".s[1]"), "{asm}");
}
