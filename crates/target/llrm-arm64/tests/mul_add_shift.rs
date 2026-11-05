//! `x*7 + n` was `lsl; sub; add` (3 instructions); the pin moves the shift to
//! the add: `sub; add w, w, x, lsl #3` (histo -Os: +5.7% instructions).

use llrm_arm64::driver::{self, Level};
use llrm_arm64::machine::Platform;

#[test]
fn a_multiply_by_seven_plus_n_folds_its_shift_into_the_add() {
    let asm = driver::assembly(include_str!("data/mul_add_shift.ll"), Platform::Linux, Level::O2).expect("lowers");
    let body = asm.lines().filter(|l| l.starts_with('\t') && !l.starts_with("\t.")).count();
    assert!(asm.contains("lsl #3"), "{asm}");
    assert_eq!(body, 3, "{asm}");
}
