//! `(x >> 29) & 3` was `lsr; and` where the pin selects one `ubfx`
//! (`tryBitfieldExtractOp` was not translated).

use llrm_arm64::driver::{self, Level};
use llrm_arm64::machine::Platform;

#[test]
fn a_masked_right_shift_is_one_ubfx() {
    let asm = driver::assembly(include_str!("data/bitfield_extract.ll"), Platform::Linux, Level::O1).expect("lowers");
    assert!(asm.contains("ubfx w0, w0, #29, #2") || asm.contains("ubfm w0, w0, #29, #30"), "{asm}");
    assert!(!asm.contains("lsr"), "{asm}");
}
