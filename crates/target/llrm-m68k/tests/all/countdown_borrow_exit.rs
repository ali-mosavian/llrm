//! A count-down loop that also steps `3n` ended in `moveq #-3, %d3; cmp.l %d3,
//! %d2; bne` on the scaled counter (`loop_countdown_sum` -O2: +17% to +20%
//! clocks over gcc, a fourth data register): the exit now tests `n` itself, and
//! `subq.l #1, %d1` leaves the borrow that is the `n == -1` test, so no `cmp`
//! stands in the loop.

use llrm_m68k::driver::{Level, assembly, cpu_of};

const COUNTDOWN: &str = include_str!("../data/countdown_scaled_sum.ll");

#[test]
fn a_count_down_loop_exits_on_the_borrow_of_its_subq() {
    for cpu in ["68000", "68020"] {
        for level in [Level::O2, Level::O3] {
            let asm = assembly(COUNTDOWN, cpu_of(cpu).unwrap(), level).expect("compiles");
            let body = asm.split("down:").nth(1).expect("the function is emitted");
            let body = body.split("\n.size").next().unwrap_or(body);
            assert!(!body.contains("cmp"), "{cpu} {level:?}: the exit compares:\n{asm}");
            assert!(body.contains("subq.l\t#1"), "{cpu} {level:?}: the counter does not step by one:\n{asm}");
            assert!(body.contains("\tbcc") || body.contains("\tbcs"), "{cpu} {level:?}: no borrow branch:\n{asm}");
        }
    }
}
