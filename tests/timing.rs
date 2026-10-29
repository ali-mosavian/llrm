//! `LLRM_DEBUG=time`: the report accounts for the whole compile.

mod common;

use std::path::Path;
use std::process::Command;

const PROGRAM: &str = "DIM s AS INTEGER\nFOR i = 1 TO 10\n s = s + i\nNEXT\nPRINT s\n";

/// llrm-qb's stderr on `PROGRAM` with `LLRM_DEBUG` set to `channels`, if any.
fn compiled(channels: Option<&str>) -> String {
    compiled_checking(channels, None)
}

/// The same with `LLRM_VERIFY` removed, or set to `verify`.
fn compiled_checking(
    channels: Option<&str>,
    verify: Option<&str>,
) -> String {
    let directory = tempfile::tempdir().expect("a directory");
    let source = directory.path().join("t.bas");
    std::fs::write(&source, PROGRAM).expect("writes the source");
    let mut command = Command::new(Path::new(env!("CARGO_BIN_EXE_llrm-qb")));
    command.args([
        source.to_str().unwrap(),
        "--dialect",
        "qb45",
        "--runtime",
        "qb45",
        "-O2",
        "-o",
        directory.path().join("t.obj").to_str().unwrap(),
    ]);
    // Every step is listed: under load, the top 30 by own time are not always
    // the same 30.
    command.env_remove("LLRM_DEBUG").env_remove("LLRM_VERIFY").env("LLRM_TIME_TOP", "1000");
    if let Some(verify) = verify {
        command.env("LLRM_VERIFY", verify);
    }
    if let Some(channels) = channels {
        command.env("LLRM_DEBUG", channels);
    }
    let done = command.output().expect("runs llrm-qb");
    assert!(done.status.success(), "{}", String::from_utf8_lossy(&done.stderr));
    String::from_utf8_lossy(&done.stderr).into_owned()
}

/// llrm-qb's stderr on `PROGRAM` compiled to assembly text (`-S`).
fn listed(channels: &str) -> String {
    let directory = tempfile::tempdir().expect("a directory");
    let source = directory.path().join("t.bas");
    std::fs::write(&source, PROGRAM).expect("writes the source");
    let done = Command::new(Path::new(env!("CARGO_BIN_EXE_llrm-qb")))
        .args([
            source.to_str().unwrap(),
            "--dialect",
            "qb45",
            "--runtime",
            "qb45",
            "-O2",
            "-S",
            "-o",
            directory.path().join("t.asm").to_str().unwrap(),
        ])
        .env_remove("LLRM_VERIFY")
        .env("LLRM_DEBUG", channels)
        .env("LLRM_TIME_TOP", "1000")
        .output()
        .expect("runs llrm-qb");
    assert!(done.status.success(), "{}", String::from_utf8_lossy(&done.stderr));
    String::from_utf8_lossy(&done.stderr).into_owned()
}

/// The number before `ms` in the `[time] <line> ...` line's field called
/// `field`.
fn field(
    report: &str,
    line: &str,
    field: &str,
) -> f64 {
    let line = report.lines().find(|one| one.starts_with(line)).unwrap_or_else(|| panic!("a `{line}` line"));
    let after = line.split(field).nth(1).unwrap_or_else(|| panic!("{field} in {line}"));
    after.trim().split_whitespace().next().unwrap().parse().unwrap()
}

/// Before the whole run was timed, about half of a compile was in no timed step
/// (#394): the outermost steps must add up to the run. By the thread's CPU
/// time, not the wall clock: on a loaded machine the thread waits for its turn
/// between two steps, which is in no step and in nothing the compiler did (4 of
/// 10 runs failed with eight busy loops on the test's CPU).
#[test]
fn test_the_outermost_steps_add_up_to_the_runs_cpu_time() {
    let report = compiled(Some("time"));
    let cpu = field(&report, "[time] cpu", "cpu");
    let untimed = field(&report, "[time] cpu", "untimed cpu");
    assert!(cpu > 0.0, "the CPU clock did not run:\n{report}");
    assert!(untimed <= (cpu * 0.02).max(2.0), "{untimed} ms of {cpu} ms of CPU is in no step:\n{report}");
}

/// A pass the pass manager ran, and the analyses it asked for, are in the
/// report, under the pipeline step.
/// llrm-cc: the object writer's step is `object write` since it writes any
/// object format, not `omf write` (llrm-qb's report, unrelated to the C front
/// end).
#[test]
fn test_a_pass_manager_pass_and_its_analyses_are_in_the_report() {
    let report = compiled(Some("time"));
    for wanted in [
        "mir gvn",
        "mir combine",
        "analysis ",
        "mir pipeline",
        "lir regalloc",
        "isel",
        "object write",
        "frontend",
        "hits",
    ] {
        assert!(report.contains(wanted), "no `{wanted}` in:\n{report}");
    }
}

/// The per-function mode names the functions the steps ran on.
#[test]
fn test_timefunc_lists_the_costliest_functions() {
    let report = compiled(Some("time,timefunc"));
    assert!(report.lines().any(|one| one.starts_with("[time] fn ")), "{report}");
    assert!(!compiled(Some("time")).lines().any(|one| one.starts_with("[time] fn ")));
}

/// Without the flag, nothing is written.
#[test]
fn test_no_time_report_without_the_flag() {
    let report = compiled(None);
    assert!(!report.contains("[time]"), "{report}");
}

/// Every pass and every machine phase was verified in a user's compile: 22 LIR
/// verifications and 8 of the MIR a function, 5% of a small program's compile
/// time; the frontend's HIR and MIR 30 M of d_faces' 3,354 M. LLVM's release
/// builds leave the verifier off; the tests, the gate and torture set
/// `LLRM_VERIFY`, and `-fverify-ir` asks for it.
#[test]
fn test_each_pass_and_phase_is_verified_only_when_asked() {
    let plain = compiled_checking(Some("time"), None);
    for step in [
        "lir verify",
        "mir verify after pass",
        "lir owned bytes",
        "mir verify pipeline",
        "hir verify",
        "mir verify frontend",
    ] {
        assert!(!plain.contains(step), "`{step}` ran without LLRM_VERIFY:\n{plain}");
    }
    let checked = compiled_checking(Some("time"), Some("1"));
    for step in [
        "lir verify",
        "mir verify after pass",
        "lir owned bytes",
        "mir verify pipeline",
        "hir verify",
        "mir verify frontend",
    ] {
        assert!(checked.contains(step), "no `{step}` with LLRM_VERIFY=1:\n{checked}");
    }
    let off = compiled_checking(Some("time"), Some("0"));
    assert!(!off.contains("lir verify"), "LLRM_VERIFY=0 is off:\n{off}");
}

/// llrm-c's stderr on a small program with `LLRM_DEBUG=time`, given `flag` and
/// the `LLRM_VERIFY` value.
fn compiled_c(
    flag: Option<&str>,
    verify: Option<&str>,
) -> String {
    let directory = tempfile::tempdir().expect("a directory");
    let source = directory.path().join("t.c");
    std::fs::write(&source, "int f(int a) { return a * 3 + 1; }\n").expect("writes the source");
    let mut command = common::cc::llrm_cc();
    command.args(["-m32", "-O2", "-o", directory.path().join("t.obj").to_str().unwrap()]);
    command.args(flag);
    command.arg(&source);
    command.env_remove("LLRM_VERIFY").env("LLRM_DEBUG", "time").env("LLRM_TIME_TOP", "1000");
    if let Some(verify) = verify {
        command.env("LLRM_VERIFY", verify);
    }
    let done = command.output().expect("runs llrm-cc");
    assert!(done.status.success(), "{}", String::from_utf8_lossy(&done.stderr));
    String::from_utf8_lossy(&done.stderr).into_owned()
}

/// The frontend's program was verified in every user's compile (30 M
/// instructions, 0.9% of d_faces -O2). It is checked under the check
/// mode, which the gate sets, and under `-fverify-ir`.
/// llrm-cc: it has no HIR, so `hir verify` is not a step; the program clang's
/// IR becomes is `mir verify frontend`. The two legs are two tests, so the
/// check mode's passes while `-fverify-ir` fails (llrm-cc hands it to clang,
/// which rejects it: a bug, reported).
#[test]
fn test_the_frontends_program_is_verified_under_the_check_mode_only() {
    let steps = ["mir verify frontend", "mir verify pipeline"];
    let plain = compiled_c(None, None);
    for step in steps {
        assert!(!plain.contains(step), "`{step}` ran in a plain compile:\n{plain}");
    }
    let checked = compiled_c(None, Some("1"));
    for step in steps {
        assert!(checked.contains(step), "no `{step}` with LLRM_VERIFY=1:\n{checked}");
    }
}

/// The same under `-fverify-ir`, the user's way to ask for the check mode.
#[test]
fn test_the_frontends_program_is_verified_under_fverify_ir() {
    let checked = compiled_c(Some("-fverify-ir"), None);
    for step in ["mir verify frontend", "mir verify pipeline"] {
        assert!(checked.contains(step), "no `{step}` with -fverify-ir:\n{checked}");
    }
}

/// With `-S` the listing writer ran outside every timed step: 21 M of a 1259 M
/// d_faces -O2 compile, and the accounting block read it as unexplained.
#[test]
fn test_the_assembly_listing_is_a_timed_step() {
    let report = listed("time");
    assert!(
        report.lines().any(|one| one.starts_with("[time]") && one.ends_with(" masm text")),
        "no `masm text` step in:\n{report}"
    );
}
