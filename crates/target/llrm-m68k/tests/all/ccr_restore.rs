//! `scc` leaves the condition codes alone, so the `move.w %d4, %ccr` after `scs` restored what
//! the codes already held. `word_compare_branches` -Os kept five such pairs of `move to ccr`
//! (2 bytes and 12 clk at 68000 each) where the emitter saved the codes round a setter that a
//! later pass moved out.

use llrm_m68k::driver::{Level, assembly, cpu_of};

const BRANCHES: &str = include_str!("../data/ccr_restore_branches.ll");

#[test]
fn the_codes_are_not_restored_straight_after_a_set_on_condition() {
    for cpu in ["68000", "68020"] {
        let asm = assembly(BRANCHES, cpu_of(cpu).unwrap(), Level::Os).expect("compiles");
        let lines: Vec<&str> = asm.lines().map(str::trim).collect();
        for pair in lines.windows(2) {
            let after_scc = pair[0].starts_with('s') && !pair[0].starts_with("sub") && pair[0].contains("\t%d");
            assert!(
                !(after_scc && pair[1].starts_with("move.w") && pair[1].ends_with("%ccr")),
                "{cpu}: `{}` then `{}`:\n{asm}",
                pair[0],
                pair[1]
            );
        }
    }
}
