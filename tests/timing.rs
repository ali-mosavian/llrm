//! `LLRM_DEBUG=time`: the report accounts for the whole compile.

use std::path::Path;
use std::process::Command;

const PROGRAM: &str = "DIM s AS INTEGER\nFOR i = 1 TO 10\n s = s + i\nNEXT\nPRINT s\n";

/// llrm-qb's stderr on `PROGRAM` with `LLRM_DEBUG` set to `channels`, if any.
fn compiled(channels: Option<&str>) -> String {
    let directory = tempfile::tempdir().expect("a directory");
    let source = directory.path().join("t.bas");
    std::fs::write(&source, PROGRAM).expect("writes the source");
    let mut command = Command::new(Path::new(env!("CARGO_BIN_EXE_llrm-qb")));
    command.args([source.to_str().unwrap(), "--dialect", "qb45", "--runtime", "qb45", "-O2", "-o", directory.path().join("t.obj").to_str().unwrap()]);
    // Every step is listed: under load, the top 30 by own time are not always the same 30.
    command.env_remove("LLRM_DEBUG").env("LLRM_TIME_TOP", "1000");
    if let Some(channels) = channels {
        command.env("LLRM_DEBUG", channels);
    }
    let done = command.output().expect("runs llrm-qb");
    assert!(done.status.success(), "{}", String::from_utf8_lossy(&done.stderr));
    String::from_utf8_lossy(&done.stderr).into_owned()
}

/// The number before `ms` in the `[time] <line> ...` line's field called `field`.
fn field(report: &str, line: &str, field: &str) -> f64 {
    let line = report.lines().find(|one| one.starts_with(line)).unwrap_or_else(|| panic!("a `{line}` line"));
    let after = line.split(field).nth(1).unwrap_or_else(|| panic!("{field} in {line}"));
    after.trim().split_whitespace().next().unwrap().parse().unwrap()
}

/// Before the whole run was timed, about half of a compile was in no timed step (#394): the
/// outermost steps must add up to the run. By the thread's CPU time, not the wall clock: on a loaded machine the
/// thread waits for its turn between two steps, which is in no step and in nothing the compiler did (4 of 10 runs failed
/// with eight busy loops on the test's CPU).
#[test]
fn test_the_outermost_steps_add_up_to_the_runs_cpu_time() {
    let report = compiled(Some("time"));
    let cpu = field(&report, "[time] cpu", "cpu");
    let untimed = field(&report, "[time] cpu", "untimed cpu");
    assert!(cpu > 0.0, "the CPU clock did not run:\n{report}");
    assert!(untimed <= (cpu * 0.02).max(2.0), "{untimed} ms of {cpu} ms of CPU is in no step:\n{report}");
}

/// A pass the pass manager ran, and the analyses it asked for, are in the report, under the
/// pipeline step.
#[test]
fn test_a_pass_manager_pass_and_its_analyses_are_in_the_report() {
    let report = compiled(Some("time"));
    for wanted in ["mir gvn", "mir fold", "analysis ", "mir pipeline", "lir regalloc", "isel", "omf write", "frontend", "hits"] {
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
