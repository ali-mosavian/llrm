//! DEVIATE TM-PAIR: the merge ahead of placement shares a two-instruction tail
//! at -Os.

use llrm_target::timings::Timings;

/// With the pin's minimum of three QB `vm/exec.c` m64 -Os grew 12,655 to 13,963
/// B (clang 13,117).
#[test]
fn the_m64_column_takes_two_instruction_tails_before_placement_at_os() {
    let timings = Timings::parse_tuned(include_str!("../src/timings.times"), llrm_target::tune::X86).unwrap();
    assert!(timings.cpu("x86-64").expect("a column").tail_merge_pairs_at_size);
}
