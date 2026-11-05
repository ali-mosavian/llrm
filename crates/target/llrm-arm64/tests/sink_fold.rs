//! `PerformSinkAndFold`: a value made before a loop and read only on one path
//! of it is made there (queens `place`, -O2: 244 B against the pin's 264 B).

use llrm_arm64::driver::{self, Level};
use llrm_arm64::machine::Platform;

fn asm() -> String {
    driver::assembly(include_str!("data/sink_fold.ll"), Platform::Linux, Level::O2).expect("lowers")
}

/// `n + 1` stayed in a callee-saved register from before the loop; it is the
/// call's argument, so it is made as `add w1, ..` beside the call.
#[test]
fn a_cheap_value_read_by_a_call_argument_is_made_at_the_call() {
    let asm = asm();
    let lines: Vec<&str> = asm.lines().map(str::trim).collect();
    let call = lines.iter().position(|line| line.starts_with("bl use")).expect("a call");
    assert!(lines[call.saturating_sub(3)..call].iter().any(|line| line.starts_with("add w1, w")), "{asm}");
}

/// `sxtw` of the index stayed a register of its own beside the store; it folds
/// into the store's extended register offset.
#[test]
fn a_sign_extension_feeding_an_index_folds_into_the_store() {
    let asm = asm();
    assert!(asm.lines().any(|line| line.contains("sxtw #2]")), "{asm}");
    assert!(!asm.lines().any(|line| line.trim().starts_with("sbfm") || line.contains("sxtw x")), "{asm}");
}
