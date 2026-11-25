//! qmove -O1: the inline trial of an external callee that stays
//! (`pl_ground_accel` into `bench_qmove`) was kept on the unit count (21
//! against 23) though the byte model said +213 B: the caller grew 126 B, the
//! callee stayed. From 2026-11-14 -O1 was judged by clocks and the copy stayed;
//! with M68K-INLINE-O1 a callee over the -O1 bar is a call at -O1.

use llrm_m68k::driver::{Level, assembly, cpu_of};

const IR: &str = include_str!("../fixtures/o1_trial_growth.ll");

/// The kernel costs more than the -O1 bar (`Tuning::inline_threshold_o1`,
/// M68K-INLINE-O1) and less than the pin's 225: a call at -O1, copied in at
/// -O2. Until 2026-11-25 -O1 copied it for its clocks (ruling 2026-11-14);
/// the clock trial does not reach it (M68K-O1-INLINE-CLOCK-TRIAL).
#[test]
fn an_o1_caller_keeps_a_call_to_a_kernel_over_the_bar_that_o2_copies_in() {
    for (level, copied) in [(Level::O1, false), (Level::O2, true)] {
        let out = assembly(IR, cpu_of("68000").unwrap(), level).expect("compiles");
        let driver = out.split("driver:").nth(1).expect("driver").split(".globl").next().unwrap();
        assert_eq!(!driver.contains("kernel"), copied, "{level:?}:\n{out}");
    }
}

const FOLDS: &str = include_str!("../fixtures/o1_trial_folds.ll");

/// Six calls of a callee with constants stay calls at -O1 (the bar refuses the
/// callee, M68K-INLINE-O1) and fold at -O2. Before the bar the -O1 trial
/// inlined them, they folded, and the object shrank (368 B against clang's 92
/// B).
#[test]
fn calls_that_fold_to_constants_stay_calls_over_the_o1_bar_and_fold_at_o2() {
    for (level, folded) in [(Level::O1, false), (Level::O2, true)] {
        let out = assembly(FOLDS, cpu_of("68000").unwrap(), level).expect("compiles");
        assert_eq!(!out.contains("(h,%pc)"), folded, "{level:?}:\n{out}");
    }
}
