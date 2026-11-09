//! The backend builds a function once and again for each optional rewrite that
//! fired (M68K-COMPILE-TRIALS: two to four full builds per function, 1031 over
//! 413 on select_of_constants), and the shipped `llrm-m68k-llc` printed no time
//! report, so the count and the cost were invisible. `LLRM_DEBUG=time` now
//! reports `m68k build` and `m68k allocate` per run and the function's own
//! `m68k fn` line; `LLRM_DEBUG=builds` names each function and the trials it
//! needed.

use std::process::Command;

fn run(channel: &str) -> String {
    let ir = concat!(env!("CARGO_MANIFEST_DIR"), "/tests/data_rt/bswap_forms.ll");
    let out = Command::new(env!("CARGO_BIN_EXE_llrm-m68k-llc"))
        .args(["-S", "-O2", "-mcpu=68000", "-o", "/dev/null", ir])
        .env("LLRM_DEBUG", channel)
        .env("LLRM_VERIFY", "0")
        .output()
        .expect("runs");
    assert!(out.status.success(), "{}", String::from_utf8_lossy(&out.stderr));
    String::from_utf8_lossy(&out.stderr).into_owned()
}

#[test]
fn the_time_report_counts_builds_and_allocations() {
    let report = run("time");
    for line in ["m68k build", "m68k allocate", "m68k fn bs"] {
        assert!(report.lines().any(|one| one.contains(line)), "no `{line}` line:\n{report}");
    }
}

#[test]
fn the_builds_channel_names_each_function_and_its_trials() {
    let report = run("builds");
    assert!(report.contains("[builds] bs trials:"), "{report}");
    assert!(report.contains("compare_destinations"), "{report}");
}
