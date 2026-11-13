//! `__divdf3` at 68000 -O2 returned 0 for 1.0 / 1.0 (the floats bench printed
//! 0, not 162635): branch folding hoisted the argument load both ways began
//! with (`move.l (64,%sp), %d7`) above the `beq`, between the `and.l #2047`
//! whose codes it reads and the branch, and the `move` overwrote them. A
//! made-again load is a `move` that sets the codes in hardware though its
//! `sets_flags` is clear. `divdf3_mid.ll` is the mid-end's output for
//! lib/fp/f64/div.c.

use llrm_m68k::driver::{Level, assembly, cpu_of};

const DIVDF3: &str = include_str!("fixtures/divdf3_mid.ll");

#[test]
fn no_move_runs_between_the_and_and_the_branch_that_reads_it() {
    for level in [Level::O1, Level::O2] {
        let out = assembly(DIVDF3, cpu_of("68000").unwrap(), level).expect("compiles");
        let lines: Vec<&str> = out.lines().map(str::trim).collect();
        for (at, line) in lines.iter().enumerate() {
            if !line.starts_with("and.l\t#2047,") {
                continue;
            }
            for next in &lines[at + 1..] {
                if next.starts_with("b") && !next.starts_with("bra") && !next.starts_with("bsr") {
                    break;
                }
                assert!(
                    !(next.starts_with("move.l\t(") && next.contains(",%sp), %d")),
                    "{level:?}: `{next}` runs between `{line}` and its branch:\n{out}"
                );
            }
        }
    }
}
