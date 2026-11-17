//! A two-lane byte vector widened to sixteen lanes by a `shufflevector` is
//! concatenated from padded pieces; `ConcatVectors` of `<2 x i8>` had no
//! legalization (`Legalize(Unsupported ConcatVectors V2I8 promote operand)`),
//! so memory_expansion -O3 failed to compile once the pin's unroll was taken.

use llrm_arm64::driver::{self, Level};
use llrm_arm64::machine::Platform;

#[test]
fn a_two_lane_byte_vector_widened_to_sixteen_lanes_lowers() {
    let asm = driver::assembly(include_str!("data/concat_short_vector.ll"), Platform::Linux, Level::O2);
    assert!(asm.is_ok(), "{asm:?}");
}
