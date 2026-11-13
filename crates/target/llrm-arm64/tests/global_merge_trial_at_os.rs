//! DEVIATE GM-TRIAL: at -Os the module is also compiled without GlobalMerge (as
//! the pin, which merges nothing there), and that is kept if it executes less
//! for the `adrp`+`add` each global needs. particle -Os: six arrays merged into
//! one base cost an `add` a trip in the hot loop, 164,544 instructions against
//! 154,252 unmerged, for 28 B.

use std::process::Command;

fn adrps(trial: bool) -> usize {
    let dir = tempfile::tempdir().unwrap();
    let (input, output) = (dir.path().join("t.ll"), dir.path().join("t.s"));
    std::fs::write(&input, include_str!("data/particle_os.ll")).unwrap();
    let mut llc = Command::new(env!("CARGO_BIN_EXE_llrm-arm64-llc"));
    llc.args(["-S", "-Os", "-o"]).arg(&output).arg(&input);
    if !trial {
        llc.env("LLRM_ARM64_NO_TRIAL", "1");
    }
    let said = llc.output().unwrap();
    assert!(said.status.success(), "{}", String::from_utf8_lossy(&said.stderr));
    let asm = std::fs::read_to_string(&output).unwrap();
    assert!(asm.contains("bench_particle:"), "premise: lowered:\n{asm}");
    asm.lines().filter(|line| line.trim_start().starts_with("adrp")).count()
}

/// Without the trial the arrays share one base (one `adrp` in `bench_particle`,
/// one in `main`).
#[test]
fn the_pass_alone_merges_the_arrays_at_os() {
    assert!(adrps(false) <= 2);
}

/// With it each array has its own base: the hot loop indexes them without an
/// `add`.
#[test]
fn the_trial_keeps_the_arrays_apart_when_that_executes_less_at_os() {
    assert!(adrps(true) >= 6);
}

/// Pricing walks every function of a compile (`to_lir` and `executed`): a
/// module nothing was noted in is compiled once, unpriced. Before, nbody -Os
/// (no loop to rotate, no merge) compiled 2.5x slower with the trials on than
/// off, for pricing a module no trial used.
#[test]
fn a_module_nothing_was_noted_in_is_not_priced_at_os() {
    use llrm_arm64::driver::{self, Level};
    use llrm_arm64::machine::Platform;
    let text = "target datalayout = \"e-m:e-p270:32:32-p271:32:32-p272:64:64-i8:8:32-i16:16:32-i64:64-i128:128-n32:64-S128-Fn32\"\n\
                target triple = \"aarch64-unknown-linux-gnu\"\n\
                define i32 @f(i32 %a, i32 %b) optsize {\n %r = add i32 %a, %b\n ret i32 %r\n}\n";
    let before = llrm_arm64::trial::priced();
    driver::assembly(text, Platform::Linux, Level::Os).expect("lowers");
    assert_eq!(llrm_arm64::trial::priced(), before);
    driver::assembly(include_str!("data/particle_os.ll"), Platform::Linux, Level::Os).expect("lowers");
    assert!(llrm_arm64::trial::priced() > before, "premise: a module that noted a merge is priced");
}

/// The model's frequencies are estimates. jpeg_fixed -Os: unmerging modelled
/// 0.004% less work (14,241,593,806 -> 14,241,052,075) and the harness counted
/// 5,497 instructions more; the model's gain there is noise, and nbody_fixed's
/// 0.3% (683,286 -> 681,179, harness -1.3%) is not.
#[test]
fn a_modelled_gain_under_the_noise_margin_does_not_unmerge() {
    use llrm_arm64::trial::{Price, keeps_unmerged};
    let jpeg = (Price { bytes: 3348, work: 14241593806.5 }, Price { bytes: 3396, work: 14241052075.1 });
    assert!(!keeps_unmerged(jpeg.0, jpeg.1, 15));
    let nbody = (Price { bytes: 400, work: 683286.0 }, Price { bytes: 404, work: 681179.0 });
    assert!(keeps_unmerged(nbody.0, nbody.1, 4));
}
