//! A lane read from a `<4 x i8>` vector (promoted to `<4 x i16>`) lowers. It
//! was refused, `Legalize(Unsupported ExtractElement V4I8 "promote operand")`,
//! so QB's `draw.c` (`draw_status`) at -O2 and -O3 had no object.

use llrm_arm64::driver::{self, Level};
use llrm_arm64::machine::Platform;

#[test]
fn a_lane_of_a_promoted_byte_vector_is_read_and_stored() {
    let asm = driver::assembly(include_str!("data/extract_lane_of_promoted_vector.ll"), Platform::Linux, Level::O2)
        .expect("lowers");
    assert_eq!(asm.matches("umov ").count(), 2, "one read per lane:\n{asm}");
    assert_eq!(asm.matches("strb ").count(), 2, "one byte store per lane:\n{asm}");
}
