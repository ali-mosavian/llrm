//! `PromoteIntOp_CONCAT_VECTORS`: two `<4 x i8>` (promoted to `<4 x i16>`)
//! joined into `<8 x i8>`. The node had no promotion and stopped selection
//! (the tail-mask shuffles of `rep_count`).

use llrm_arm64::driver::{self, Level};
use llrm_arm64::machine::Platform;

#[test]
fn two_promoted_byte_vectors_are_joined() {
    let asm = driver::assembly(include_str!("data/concat_promoted.ll"), Platform::Linux, Level::O2).expect("lowers");
    assert!(asm.contains("str"), "{asm}");
}
