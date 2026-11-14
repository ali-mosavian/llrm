//! qmove -O1: the inline trial of an external callee that stays
//! (`pl_ground_accel` into `bench_qmove`) was kept on the unit count (21
//! against 23) though the byte model said +213 B: the caller grew 126 B, the
//! callee stayed. Since 2026-11-14 -O1 is judged by clocks and the copy stays.

use llrm_m68k::driver::{Level, assembly, cpu_of};

const IR: &str = include_str!("fixtures/o1_trial_growth.ll");

/// -O1 is judged by clocks (user ruling 2026-11-14): the copy saves the call and the kernel's frame setup.
#[test]
fn an_o1_caller_copies_in_an_external_kernel_for_its_clocks() {
    let out = assembly(IR, cpu_of("68000").unwrap(), Level::O1).expect("compiles");
    let driver = out.split("driver:").nth(1).expect("driver").split(".globl").next().unwrap();
    assert!(!driver.contains("kernel"), "the kernel stayed a call:\n{out}");
}

const FOLDS: &str = include_str!("fixtures/o1_trial_folds.ll");

/// uint64_to_float -O1: six calls of `classify` with constants stayed calls
/// (368 B, clang 92 B) because the size bound refuses the callee; the trial
/// inlines them, they fold, and the object shrinks.
#[test]
fn an_o1_caller_whose_calls_fold_to_constants_inlines_them() {
    let out = assembly(FOLDS, cpu_of("68000").unwrap(), Level::O1).expect("compiles");
    assert!(!out.contains("(h,%pc)"), "the calls to h stayed:\n{out}");
}
