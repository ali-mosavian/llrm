//! A `shufflevector` of two short vectors into a longer result pads both
//! inputs with undefined lanes; the second input's mask lanes follow the
//! first's padded length (`visitShuffleVector`, "Readjust mask for new input
//! vector length"). They were not moved, so a zip of two `<8 x i8>` read the
//! first input's undefined padding for the second input: the second load was
//! never made and the result's odd bytes were garbage.

use llrm_arm64::driver::{self, Level};
use llrm_arm64::machine::Platform;

#[test]
fn the_second_input_of_a_padded_shuffle_is_read() {
    let asm =
        driver::assembly(include_str!("data/shuffle_padded_second.ll"), Platform::Linux, Level::O2).expect("lowers");
    assert!(asm.contains("[x1]"), "the second input (x1) is never loaded:\n{asm}");
}
