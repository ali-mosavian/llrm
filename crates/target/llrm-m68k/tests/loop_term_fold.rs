//! `LoopTermFold` at `-Os`: `partition`'s loop counted up and compared
//! (`addq.l #1, %d3; cmp.l %d2, %d3; blt`) beside a pointer that already
//! walks the array. The fold gives the exit test to the pointer's other
//! counter, which ends at zero: 102 -> 98 B and 1374 -> 1272 clk at 68000.

use llrm_m68k::driver::{Level, assembly, cpu_of};

const PARTITION: &str = include_str!("fixtures/partition_os_cc.ll");

/// The mnemonic of the last backward branch, the loop's latch.
fn latch_branch(listing: &str) -> String {
    let mut seen = Vec::new();
    let mut latch = None;
    for line in listing.lines() {
        if let Some(label) = line.strip_suffix(':') {
            seen.push(label.to_owned());
        } else if let Some((mnemonic, target)) = line.trim().split_once('\t')
            && mnemonic.starts_with('b')
            && seen.iter().any(|label| label == target)
        {
            latch = Some(mnemonic.to_owned());
        }
    }
    latch.expect("a loop")
}

#[test]
fn the_os_latch_ends_on_a_counter_at_zero() {
    for cpu in ["68000", "68020"] {
        let listing = assembly(PARTITION, cpu_of(cpu).unwrap(), Level::Os).expect("compiles");
        assert_eq!(latch_branch(&listing), "bne", "{cpu}:\n{listing}");
    }
}
