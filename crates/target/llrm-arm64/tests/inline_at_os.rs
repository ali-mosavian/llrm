//! At -Os an inline does not grow the module.

use llrm_arm64::driver::{self, Level};
use llrm_arm64::machine::Platform;

/// jpeg_fixed -Os: `put32`, a 4-trip store loop, was copied into each of its
/// eight callers (the pin's `OptSizeThreshold` 50 allows growth), 176 B over
/// gcc, which keeps it a function.
#[test]
fn a_store_loop_called_from_six_sites_is_not_copied_into_each_site_at_os() {
    let asm = driver::assembly(include_str!("data/store_loop_sites.ll"), Platform::Linux, Level::Os).expect("lowers");
    let calls = asm.lines().filter(|line| line.trim().starts_with("bl ") && line.contains("put32")).count();
    let copies = asm.lines().filter(|line| line.trim().starts_with("strb ")).count();
    assert!(calls >= 1 && copies <= 2, "{calls} calls, {copies} copies of the loop: {asm}");
}

/// jpeg_fixed -Os: `tmul`, a multiply-divide the unit model refuses at each
/// site, was never tried inline because its callers are over 300 instructions
/// (the byte trial's bound); the copies are smaller than the calls and the
/// function (jpeg_fixed 3616 -> 3516 B).
#[test]
fn small_helpers_of_a_large_caller_are_tried_inline_at_os() {
    let asm = driver::assembly(include_str!("data/jpeg_fixed_os.ll"), Platform::Linux, Level::Os).expect("lowers");
    let kept: Vec<&str> = ["tmul"].into_iter().filter(|name| asm.lines().any(|line| *line == format!("{name}:"))).collect();
    assert!(kept.is_empty(), "left as functions: {kept:?}");
}

/// jpeg_fixed -Os: a callee the byte trial rejected was tried again in the next
/// round of the same caller (`message`, `next16` and `put32` twice each), each
/// trial a simplifier run over the caller: 0.38 s against 0.14 s.
#[test]
fn a_callee_the_trial_rejected_is_not_tried_again_in_the_same_caller() {
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
    let mut rejected = std::collections::HashSet::new();
    let mut trials = 0;
    for line in log.lines() {
        let Some((caller, rest)) = line.split_once(' ') else { continue };
        if let Some(callee) = line.split_once(" SZ2 trial rejected '").and_then(|(_, rest)| rest.strip_suffix('\'')) {
            rejected.insert((caller.to_owned(), callee.to_owned()));
        } else if let Some((_, callee)) = rest.rsplit_once("call(s) to '").filter(|_| line.contains("SZ2 trial of")) {
            trials += 1;
            let callee = callee.split('\'').next().unwrap();
            assert!(!rejected.contains(&(caller.to_owned(), callee.to_owned())), "{caller} tried {callee} again:\n{log}");
        }
    }
    assert!(trials > 0, "premise: trials run\n{log}");
}

/// The SZ2 trial remarks of `jpeg_fixed_os.ll` at -Os, in order.
fn jpeg_fixed_trials() -> Vec<String> {
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
    let trials: Vec<String> = log.lines().filter(|line| line.contains("SZ2 trial of")).map(str::to_owned).collect();
    assert!(!trials.is_empty(), "premise: trials run\n{log}");
    trials
}

/// The callee a trial remark names, with its caller.
fn trial_site(line: &str) -> (&str, &str) {
    let caller = line.split(' ').next().unwrap();
    let callee = line.rsplit_once("call(s) to '").unwrap().1.split('\'').next().unwrap();
    (caller, callee)
}

/// jpeg_fixed -Os: every SZ2 trial ran the whole simplifier list over a 600-instruction caller,
/// 2.95 G instructions to compile it against 1.38 G before the trial existed. A trial's copy
/// goes through the cleanup list first; only a near miss runs the whole list (2.05 G).
#[test]
fn the_byte_trial_simplifies_its_copy_with_the_cleanup_list() {
    let trials = jpeg_fixed_trials();
    let light = trials.iter().filter(|line| line.contains("cleanup list")).count();
    assert!(light > trials.len() - light, "most trials use the cleanup list: {trials:#?}");
    for (at, line) in trials.iter().enumerate().filter(|(_, line)| !line.contains("cleanup list")) {
        let site = trial_site(line);
        assert!(
            trials[..at].iter().any(|before| before.contains("cleanup list") && trial_site(before) == site),
            "whole-list trial without a cleanup-list trial first: {line}\n{trials:#?}"
        );
    }
}

/// jpeg_single -Os: the cleanup list rejected `extend`, `receive` and `clamp` by a few bytes the
/// whole list removes, leaving 3608 B against 3592 B. A trial rejected within `NEAR_MISS` is run
/// again with the whole list.
#[test]
fn a_trial_the_cleanup_list_misses_by_a_little_is_run_again_with_the_whole_list() {
    let trials = jpeg_fixed_trials();
    assert!(
        trials.iter().any(|line| !line.contains("cleanup list") && line.contains("'clamp'")),
        "premise: clamp is a near miss\n{trials:#?}"
    );
}
