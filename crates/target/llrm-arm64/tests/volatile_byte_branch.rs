//! A volatile byte load compared with zero is a zero-extending load and a
//! `cbz`, as in the pin; it was `ldrb; tst w, #0xff; b.eq`.

use llrm_arm64::driver;
use llrm_arm64::machine::Platform;

#[test]
fn a_volatile_byte_load_branches_with_cbz() {
    let asm = driver::assembly(include_str!("data/volatile_byte_branch.ll"), Platform::Linux, true).expect("lowers");
    assert_eq!(asm.matches("ldrb").count(), 1, "{asm}");
    assert!(asm.contains("cbz w") || asm.contains("cbnz w"), "{asm}");
    assert!(!asm.contains("tst") && !asm.contains("ands") && !asm.contains("and w"), "{asm}");
}
