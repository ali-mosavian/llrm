//! At -Os an inline does not grow the module.

use llrm_arm64::driver::{self, Level};
use llrm_arm64::machine::Platform;

/// jpeg_fixed -Os: `put32`, a 4-trip store loop, is priced -10 by the pin
/// (below zero, so it counts as shrinking the caller) and `opt -O2` copies it
/// into all six sites. The size guard gave way to that (#9598); the -Os trial
/// (INLINEHINT-GROWTH-TRIAL) now compiles the module with the calls kept and
/// keeps that object where it is smaller: jpeg_fixed 4,020 -> 3,956 B and 0.5%
/// fewer instructions executed (gcc keeps `put32` a function too).
#[test]
fn a_store_loop_the_pin_prices_below_zero_stays_a_call_where_the_object_is_smaller() {
    let asm = driver::assembly(include_str!("data/store_loop_sites.ll"), Platform::Linux, Level::Os).expect("lowers");
    let calls = asm.lines().filter(|line| line.trim().starts_with("bl ") && line.contains("put32")).count();
    assert!(calls >= 5 && asm.lines().any(|line| line == "put32:"), "{calls} calls: {asm}");
}

/// jpeg_fixed -Os: `tmul`, a multiply-divide the unit model refuses at each
/// site, was never tried inline because its callers are over 300 instructions
/// (the byte trial's bound); the copies are smaller than the calls and the
/// function (jpeg_fixed 3616 -> 3516 B).
#[test]
fn small_helpers_of_a_large_caller_are_tried_inline_at_os() {
    let asm = driver::assembly(include_str!("data/jpeg_fixed_os.ll"), Platform::Linux, Level::Os).expect("lowers");
    let kept: Vec<&str> =
        ["tmul"].into_iter().filter(|name| asm.lines().any(|line| *line == format!("{name}:"))).collect();
    assert!(kept.is_empty(), "left as functions: {kept:?}");
}

/// jpeg_fixed -Os: a callee the byte trial rejected was tried again in the next
/// round of the same caller (`message`, `next16` and `put32` twice each), each
/// trial a simplifier run over the caller: 0.38 s against 0.14 s. One compile
/// only (`LLRM_ARM64_NO_TRIAL`): the -Os trials (rotation, merge, site) compile
/// the module again, and each compile tries `message` afresh, which the log
/// showed as four tries of one callee.
#[test]
fn a_callee_the_trial_rejected_is_not_tried_again_in_the_same_caller() {
    let dir = tempfile::tempdir().unwrap();
    let ir = dir.path().join("jpeg_fixed_os.ll");
    std::fs::write(&ir, include_str!("data/jpeg_fixed_os.ll")).unwrap();
    let output = std::process::Command::new(env!("CARGO_BIN_EXE_llrm-arm64-llc"))
        .env("LLRM_INLINE_REMARKS", "1")
        .env("LLRM_ARM64_NO_TRIAL", "1")
        .arg("-Os")
        .arg("-o")
        .arg(dir.path().join("o.o"))
        .arg(&ir)
        .output()
        .unwrap();
    let log = String::from_utf8_lossy(&output.stderr);
    let mut rejected = llrm_support::hash::HashSet::default();
    let mut trials = 0;
    for line in log.lines() {
        let Some((caller, rest)) = line.split_once(' ') else { continue };
        if let Some(callee) = line.split_once(" SZ2 trial rejected '").and_then(|(_, rest)| rest.strip_suffix('\'')) {
            rejected.insert((caller.to_owned(), callee.to_owned()));
        } else if let Some((_, callee)) = rest.rsplit_once("call(s) to '").filter(|_| line.contains("SZ2 trial of")) {
            trials += 1;
            let callee = callee.split('\'').next().unwrap();
            assert!(
                !rejected.contains(&(caller.to_owned(), callee.to_owned())),
                "{caller} tried {callee} again:\n{log}"
            );
        }
    }
    assert!(trials > 0, "premise: trials run\n{log}");
}

/// jpeg_fixed -Os: every SZ2 trial ran the whole simplifier list over a
/// 600-instruction caller, 2.95 G instructions to compile it against 1.38 G
/// before the trial existed. The trial's copy goes through the cleanup list
/// (1.39 G); `tmul` is still inlined.
#[test]
fn the_byte_trial_simplifies_its_copy_with_the_cleanup_list() {
    let dir = tempfile::tempdir().unwrap();
    let ir = dir.path().join("jpeg_fixed_os.ll");
    std::fs::write(&ir, include_str!("data/jpeg_fixed_os.ll")).unwrap();
    let output = std::process::Command::new(env!("CARGO_BIN_EXE_llrm-arm64-llc"))
        .env("LLRM_INLINE_REMARKS", "1")
        .arg("-Os")
        .arg("-o")
        .arg(dir.path().join("o.o"))
        .arg(&ir)
        .output()
        .unwrap();
    let log = String::from_utf8_lossy(&output.stderr);
    let trials: Vec<&str> = log.lines().filter(|line| line.contains("SZ2 trial of")).collect();
    assert!(!trials.is_empty(), "premise: trials run\n{log}");
    let whole: Vec<&&str> = trials.iter().filter(|line| !line.contains("cleanup list")).collect();
    assert!(whole.is_empty(), "trials on the whole list: {whole:?}");
}
