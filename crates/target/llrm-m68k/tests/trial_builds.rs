//! M68K-COMPILE-TRIALS: a function whose sign-extended-word add (`suba.w`)
//! fired was built twice, once more without the rewrite, to price it; the pass
//! takes the cpu and the price itself (`sextacc`), so the second build bought
//! nothing. `LLRM_DEBUG=builds` names each build: this function now has one.

use std::process::Command;

const IR: &str = include_str!("sext_acc_ir.ll");

fn report(cpu: &str) -> String {
    let dir = std::path::Path::new(env!("CARGO_TARGET_TMPDIR"));
    let ir = dir.join(format!("trial-builds-{}.ll", std::process::id()));
    std::fs::write(&ir, IR).unwrap();
    let out = Command::new(env!("CARGO_BIN_EXE_llrm-m68k-llc"))
        .args(["-S", "-O2", &format!("-mcpu={cpu}"), "-o", "/dev/null"])
        .arg(&ir)
        .env("LLRM_DEBUG", "builds")
        .env("LLRM_VERIFY", "0")
        .output()
        .expect("runs");
    std::fs::remove_file(&ir).ok();
    assert!(out.status.success(), "{}", String::from_utf8_lossy(&out.stderr));
    String::from_utf8_lossy(&out.stderr).into_owned()
}

#[test]
fn the_sext_add_is_not_a_trial_and_costs_no_build() {
    // the 68000 takes `suba.w` here (tests/sext_acc.rs)
    let report = report("68000");
    let trials = report.lines().find(|line| line.starts_with("[builds] f trials:")).expect("trials line");
    assert!(!trials.contains("sext_adds"), "{trials}");
    let builds = report.lines().filter(|line| line.trim_end() == "[builds] f").count();
    let fired = trials.matches(" true").count();
    assert_eq!(builds, 1 + fired, "one build, and one more for each trial that fired:\n{report}");
}
