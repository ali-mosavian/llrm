//! `addTypeForNEON` makes the conversions of a 64-bit vector custom
//! (`LowerVectorINT_TO_FP`), so `uitofp <2 x i32> to <2 x double>` is a `ushll`
//! and a `ucvtf`. The 64-bit types had no action: the node reached selection
//! and `Legalize` refused it (`CannotSelect("UIntToFp")`), which stopped the
//! SLP-vectorized matmul and `object_extent_loop`.

use llrm_arm64::driver::{self, Level};
use llrm_arm64::machine::Platform;

#[test]
fn a_conversion_of_a_two_lane_i32_vector_to_double_is_widened_then_converted() {
    let asm = driver::assembly(include_str!("data/dreg_int_to_fp.ll"), Platform::Linux, Level::O2).expect("lowers");
    assert!(asm.contains("ushll") && asm.contains("ucvtf"), "{asm}");
    assert!(asm.contains("sshll") && asm.contains("scvtf"), "{asm}");
}
