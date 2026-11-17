//! `SplitVecRes_BITCAST`: a `<64 x i8>` cast to `<16 x i32>`, both split (the tbl
//! shuffle of `zext <16 x i8> to <16 x i32>` is one). The node had no split and
//! refused the function (`Unsupported { op: BitCast, what: "split result" }`).

use llrm_arm64::driver::{self, Level};
use llrm_arm64::machine::Platform;

#[test]
fn a_bitcast_of_a_split_vector_is_cast_by_halves() {
    let asm = driver::assembly(include_str!("data/bitcast_split.ll"), Platform::Linux, Level::O2).expect("lowers");
    assert!(asm.contains("add"), "{asm}");
}
