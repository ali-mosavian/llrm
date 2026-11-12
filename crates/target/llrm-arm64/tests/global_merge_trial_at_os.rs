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
