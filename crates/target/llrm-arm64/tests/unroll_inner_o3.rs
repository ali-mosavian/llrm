//! At -O3 the pin decides an innermost loop inside a loop of known trips by
//! its size alone (DEVIATE ARM64-NEST-O3-INNER). Priced, fpbench's 6-trip inner
//! loop was vetoed (29 clocks for 217 B) while the 6-trip loop around it was
//! unrolled, and the six copies of the inner loop were then unrolled unpriced:
//! 575 instructions, 2,300 B. The pin unrolls the inner loops and leaves the
//! outer one rolled: 353 instructions, 1,412 B.

use llrm_arm64::driver::{self, Level};
use llrm_arm64::machine::Platform;

#[test]
fn fpbench_o3_unrolls_the_inner_loops_and_leaves_the_outer_one_rolled() {
    let asm = driver::assembly(include_str!("data/fpbench_o3.ll"), Platform::Linux, Level::O3).expect("lowers");
    let insns = asm.lines().filter(|line| line.starts_with('\t') && !line.starts_with("\t.")).count();
    assert!(insns <= 400, "{insns} instructions");
}
