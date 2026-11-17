//! Three byte stores at `3 * i + k` are one `st3` of sixteen pixels.

use llrm_arm64::driver::{self, Level};
use llrm_arm64::machine::Platform;

/// jpeg_fixed's convert, O3: the loop vectorized to `<48 x i8>` stores. llc
/// first failed on them ("widen operand" of an `extract_subvector` of 48
/// lanes); then it built each member lane by lane (`umov`, `ins`: 16 inserts
/// a member, 64 instructions a loop). The pin takes the members as the three
/// concatenated vectors and `st3` stores them as they are.
#[test]
fn a_store_of_three_interleaved_members_is_one_st3_of_the_members() {
    let asm = driver::assembly(include_str!("data/interleave_triple_store.ll"), Platform::Linux, Level::O3)
        .expect("lowers");
    assert!(asm.contains("st3 {"), "{asm}");
    assert!(!asm.contains("\tins ") && !asm.contains("\tumov "), "{asm}");
}
