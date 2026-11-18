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
    for line in ["m68k build", "m68k allocate", "m68k fn "] {
        assert!(report.lines().any(|one| one.contains(line)), "no `{line}` line:\n{report}");
    }
}

#[test]
fn the_builds_channel_names_each_function_and_its_trials() {
    let report = run("builds");
    assert!(report.contains("[builds] bs trials:"), "{report}");
    assert!(report.contains("compare_destinations"), "{report}");
}

/// `llrm-cc --dump DIR -c` wrote no stage files at m68k: only the `-S` path
/// took the directory, so a same-IR pin comparison needed a detour through
/// `-S`.
#[test]
fn an_object_build_writes_the_stage_dump_as_the_assembly_build_does() {
    let ir = concat!(env!("CARGO_MANIFEST_DIR"), "/tests/data_rt/bswap_forms.ll");
    let dir = std::path::Path::new(env!("CARGO_TARGET_TMPDIR")).join(format!("dump-c-{}", std::process::id()));
    let status = Command::new(env!("CARGO_BIN_EXE_llrm-m68k-llc"))
        .args(["-c", "-O2", "-mcpu=68000", "-o"])
        .arg(dir.with_extension("o"))
        .arg(format!("--dump={}", dir.display()))
        .arg(ir)
        .env("LLRM_VERIFY", "0")
        .status()
        .expect("runs");
    assert!(status.success());
    let stages = std::fs::read_dir(&dir).map(|entries| entries.count()).unwrap_or(0);
    std::fs::remove_dir_all(&dir).ok();
    std::fs::remove_file(dir.with_extension("o")).ok();
    assert!(stages > 10, "{stages} stage files");
}

/// M68K-ONE-BUILD: a -O2 function whose multiply is a chain (the multiply
/// trial's build) was built twice more; matmul -O2 68000 took 4472 M
/// instructions, twice what one build takes.
#[test]
fn a_speed_level_multiply_chain_adds_no_trial_builds() {
    let ir = "target datalayout = \"E-m:e-p:32:16:32-i8:8:8-i16:16:16-i32:16:32-n8:16:32-a:0:16-S16\"\ndefine i32 @f(ptr %a, i32 %n) {\nentry:\n  br label %loop\nloop:\n  %i = phi i32 [ 0, %entry ], [ %i1, %loop ]\n  %s = phi i32 [ 0, %entry ], [ %s1, %loop ]\n  %p = getelementptr i32, ptr %a, i32 %i\n  %x = load i32, ptr %p\n  %m = mul i32 %x, 1000\n  %s1 = add i32 %s, %m\n  %i1 = add i32 %i, 1\n  %d = icmp eq i32 %i1, %n\n  br i1 %d, label %out, label %loop\nout:\n  ret i32 %s1\n}\n";
    let path = std::path::Path::new(env!("CARGO_TARGET_TMPDIR")).join(format!("one-build-{}.ll", std::process::id()));
    std::fs::write(&path, ir).unwrap();
    let out = Command::new(env!("CARGO_BIN_EXE_llrm-m68k-llc"))
        .args(["-S", "-O2", "-mcpu=68020", "-o", "/dev/null"])
        .arg(&path)
        .env("LLRM_DEBUG", "builds")
        .env("LLRM_VERIFY", "0")
        .output()
        .expect("runs");
    std::fs::remove_file(&path).ok();
    let report = String::from_utf8_lossy(&out.stderr);
    assert!(report.contains("mul_trial true"), "the multiply was not a chain:\n{report}");
    let builds = report.lines().filter(|line| line.trim_end() == "[builds] f").count();
    // the one build: no relaxed-class, shared-CSE or slot-register build
    assert!(builds <= 1, "{builds} builds:\n{report}");
}

/// A -Os function whose constants were rematerialised was built a third time
/// to try them off (`bs_high`: 2 builds); over 165 programs that build was
/// never smaller, and it cost 11% of the corpus's compile instructions.
#[test]
fn a_size_build_does_not_try_rematerialisation_off() {
    let ir = concat!(env!("CARGO_MANIFEST_DIR"), "/tests/data_rt/bswap_forms.ll");
    let out = Command::new(env!("CARGO_BIN_EXE_llrm-m68k-llc"))
        .args(["-S", "-Os", "-mcpu=68020", "-o", "/dev/null", ir])
        .env("LLRM_DEBUG", "builds")
        .env("LLRM_VERIFY", "0")
        .output()
        .expect("runs");
    assert!(out.status.success());
    let report = String::from_utf8_lossy(&out.stderr);
    assert!(
        report.contains("[builds] bs_high trials:") && report.contains("remat_constants true"),
        "premise: {report}"
    );
    let builds = report.lines().filter(|line| *line == "[builds] bs_high").count();
    // the held-constants build of #10069 is the one second build; a
    // rematerialisation-off build would be a third
    assert!(builds <= 2, "bs_high built {builds} times:\n{report}");
}
