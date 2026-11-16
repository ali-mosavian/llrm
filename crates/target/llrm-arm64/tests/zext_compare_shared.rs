//! `SimplifySetCC` compares a zero extension's input, whoever else reads the
//! extension (TargetLowering.cpp L5105). grep -O2: the compare of a loaded
//! byte was `orr w11, wzr, w9; subs xzr, x11, #0` beside an add that read the byte: one
//! instruction more per iteration than gcc's 18.

use llrm_arm64::driver::{self, Level};
use llrm_arm64::machine::Platform;

/// The byte is read by an add and a compare; the compare is made on the
/// loaded register, with no copy to widen it.
#[test]
fn a_compare_of_a_shared_zero_extension_is_made_on_its_input() {
    let asm = driver::assembly(include_str!("data/zext_compare_shared.ll"), Platform::Linux, Level::O2).expect("lowers");
    let body: Vec<&str> = asm.lines().skip_while(|line| *line != "f:").take_while(|line| !line.starts_with("\tret")).collect();
    let text = body.join("\n");
    assert!(!text.contains("orr w"), "{text}");
    assert!(text.contains("subs wzr, w"), "{text}");
}
