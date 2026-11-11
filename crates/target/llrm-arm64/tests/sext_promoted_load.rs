//! A promoted `sra` operand was `shl 16; sra 16` on an any-extending load
//! (`ldrh`) where the pin makes `sext_inreg`, which folds into `ldrsh`:
//! `sext(sdiv i16 x, 4)` was 8 instructions, the pin's 5 (stft_fixed -O1,
//! +1.2% instructions).

use llrm_arm64::driver::{self, Level};
use llrm_arm64::machine::Platform;

#[test]
fn a_promoted_signed_shift_operand_loads_with_ldrsh() {
    let asm = driver::assembly(include_str!("data/sext_promoted_load.ll"), Platform::Linux, Level::O2).expect("lowers");
    assert!(asm.contains("ldrsh"), "{asm}");
    assert!(!asm.contains("#16"), "{asm}");
}
