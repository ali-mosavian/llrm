//! DEVIATE TM-PAIR: the merge ahead of placement shares a two-instruction tail
//! at -Os.

use llrm_target::timings::Timings;

/// With the pin's minimum of three QB `vm/exec.c` m32 -Os grew 11,700 to 13,239
/// B (clang -march=i486 12,318).
#[test]
fn the_m32_columns_take_two_instruction_tails_before_placement_at_os() {
    let timings = Timings::parse_tuned(include_str!("../src/timings.times"), llrm_target::tune::X86).unwrap();
    for cpu in ["486", "P5"] {
        assert!(timings.cpu(cpu).expect("a column").tail_merge_pairs_at_size, "{cpu}");
    }
}
