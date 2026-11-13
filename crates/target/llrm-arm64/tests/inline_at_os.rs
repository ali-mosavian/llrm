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
    let kept: Vec<&str> =
        ["tmul"].into_iter().filter(|name| asm.lines().any(|line| *line == format!("{name}:"))).collect();
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

/// vla -O2: one site of `vla` was rejected, and the trial of all its sites (the
/// callee goes with them: more credit) was skipped as a retry, so the nest's
/// calls with constant sizes were never priced as a whole.
#[test]
fn the_group_of_a_callees_sites_is_tried_after_one_site_was_rejected() {
    let dir = tempfile::tempdir().unwrap();
    let ir = dir.path().join("vla.ll");
    std::fs::write(&ir, include_str!("darwin/vla.ll")).unwrap();
    let output = std::process::Command::new(env!("CARGO_BIN_EXE_llrm-arm64-llc"))
        .env("LLRM_INLINE_REMARKS", "1")
        .arg("-O2")
        .arg("-o")
        .arg(dir.path().join("o.o"))
        .arg(&ir)
        .output()
        .unwrap();
    let log = String::from_utf8_lossy(&output.stderr);
    let sizes: Vec<usize> = log
        .lines()
        .filter(|line| line.contains("SZ2 trial of") && line.contains("call(s) to 'vla'"))
        .filter_map(|line| line.split("trial of ").nth(1)?.split(' ').next()?.parse().ok())
        .collect();
    assert!(sizes.contains(&1), "premise: a single site is tried\n{log}");
    assert!(sizes.iter().any(|&n| n > 1), "no trial of the callee's group:\n{log}");
}
