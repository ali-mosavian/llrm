//! `GlobalMerge` leaves a global alone when a loop indexes it by a value the
//! loop loads (DEVIATE ARM64-GM-INDEXED). grep -O3: the merged tables cost
//! `add xT, base, idx; ldrb [xT, #off]` per read, 20 instructions per
//! iteration against 19 at -O2 and gcc's 18; 2.1M of 22M instructions.

use llrm_arm64::driver::{self, Level};
use llrm_arm64::machine::Platform;

fn body(name: &str) -> String {
    let asm = driver::assembly(include_str!("data/global_merge_indexed.ll"), Platform::Linux, Level::O3).expect("lowers");
    let mut lines = asm.lines().skip_while(|line| *line != format!("{name}:"));
    let first = lines.next().expect("the function is there");
    let rest = lines.take_while(|line| line.starts_with('\t') || line.starts_with(".L"));
    std::iter::once(first).chain(rest).collect::<Vec<_>>().join("\n")
}

/// Two tables read through a loaded byte stayed one merged struct: each read
/// was an `add` and an offset load.
#[test]
fn tables_indexed_by_a_loaded_value_stay_apart() {
    let text = body("data");
    assert!(!text.contains("_MergedGlobals"), "{text}");
}

/// Indexed by the induction variable, the merge still happens (as the pin).
#[test]
fn tables_indexed_by_the_induction_variable_are_merged() {
    let text = body("steps");
    assert!(text.contains("_MergedGlobals"), "{text}");
}
