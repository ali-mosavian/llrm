//! qmove -O1: the inline trial of an external callee that stays
//! (`pl_ground_accel` into `bench_qmove`) was kept on the unit count (21
//! against 23) though the byte model said +213 B: the caller grew 126 B, the
//! callee stayed. -O1 is strict per axis, so a copy that grows the code is
//! refused.

use llrm_m68k::driver::{Level, assembly, cpu_of};

const IR: &str = include_str!("fixtures/o1_trial_growth.ll");

fn lines_of(
    function: &str,
    out: &str,
) -> usize {
    let mut inside = false;
    let mut n = 0;
    for line in out.lines() {
        if line.starts_with(function) && line.trim_end().ends_with(':') {
            inside = true;
        } else if inside && !line.starts_with('\t') && line.ends_with(':') && !line.starts_with(".L") {
            inside = false;
        }
        if inside && line.starts_with('\t') && !line.trim_start().starts_with('.') {
            n += 1;
        }
    }
    n
}

#[test]
fn an_o1_caller_keeps_the_call_to_an_external_kernel_it_would_grow_by() {
    let out = assembly(IR, cpu_of("68000").unwrap(), Level::O1).expect("compiles");
    let driver = lines_of("driver", &out);
    let kernel = lines_of("kernel", &out);
    assert!(driver * 2 < kernel, "driver {driver} instructions against kernel {kernel}: the kernel was copied in");
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
