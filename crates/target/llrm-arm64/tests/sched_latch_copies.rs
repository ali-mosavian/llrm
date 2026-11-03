//! A loop's new values are made after the last read of the old ones, so each
//! takes the old one's register.

use llrm_arm64::driver;
use llrm_arm64::machine::Platform;

/// Six values carried round a loop were each made before the old value's last
/// read, so the allocator moved them at the latch: 7 register moves in
/// `bench_recmany` (QB `recmany`: 39034 instructions where main ran 34530, the
/// new schedule 4).
#[test]
fn values_carried_round_a_loop_are_made_into_the_registers_they_replace() {
    let asm = driver::assembly(include_str!("data/sched_latch_copies.ll"), Platform::Linux, true).expect("lowers");
    let body = asm.split("bench_recmany:").nth(1).and_then(|rest| rest.split("\nmain:").next()).expect("the function");
    // A register move prints as `orr xd, xzr, xm, lsl #0`.
    let movs = body.lines().filter(|line| line.trim_start().starts_with("orr ") && line.contains(", xzr, ")).count();
    assert!(movs > 0 && movs <= 4, "{movs} moves\n{asm}");
}
