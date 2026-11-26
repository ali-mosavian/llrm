//! `LLRM_DEBUG=time` printed nothing from llrm-arm64-llc: its `main` never ran
//! the time report (the m68k one does), so a per-pass compile-time profile of
//! arm64 had to be read from `perf` by symbol.

use std::process::Command;

#[test]
fn the_time_channel_reports_the_steps_of_a_compile() {
    let dir = tempfile::tempdir().unwrap();
    let input = dir.path().join("t.ll");
    std::fs::write(
        &input,
        "target datalayout = \"e-m:e-p270:32:32-p271:32:32-p272:64:64-i8:8:32-i16:16:32-i64:64-i128:128-n32:64-S128-Fn32\"\n\
         target triple = \"aarch64-unknown-linux-gnu\"\n\
         define i32 @f(i32 %a, i32 %b) {\n %r = add i32 %a, %b\n ret i32 %r\n}\n",
    )
    .unwrap();
    let said = Command::new(env!("CARGO_BIN_EXE_llrm-arm64-llc"))
        .env("LLRM_DEBUG", "time")
        .args(["-O2", "-o"])
        .arg(dir.path().join("t.o"))
        .arg(&input)
        .output()
        .unwrap();
    let log = String::from_utf8_lossy(&said.stderr);
    assert!(said.status.success() && log.contains("[time] by own time:"), "{log}");
}

/// The report's `outside every step` held the whole backend (37% of one
/// module's compile): each stage of it is a step now.
#[test]
fn the_backend_stages_are_steps_of_the_report() {
    let dir = tempfile::tempdir().unwrap();
    let input = dir.path().join("t.ll");
    std::fs::write(
        &input,
        "target datalayout = \"e-m:e-p270:32:32-p271:32:32-p272:64:64-i8:8:32-i16:16:32-i64:64-i128:128-n32:64-S128-Fn32\"\n\
         target triple = \"aarch64-unknown-linux-gnu\"\n\
         define i32 @f(i32 %a, i32 %b) {\n %r = add i32 %a, %b\n ret i32 %r\n}\n",
    )
    .unwrap();
    let said = Command::new(env!("CARGO_BIN_EXE_llrm-arm64-llc"))
        .env("LLRM_DEBUG", "time")
        .env("LLRM_TIME_TOP", "500")
        .args(["-O2", "-o"])
        .arg(dir.path().join("t.o"))
        .arg(&input)
        .output()
        .unwrap();
    let log = String::from_utf8_lossy(&said.stderr);
    for step in [
        "arm64 parse IR",
        "arm64 select",
        "arm64 machine passes before allocation",
        "arm64 register allocation",
        "arm64 lowering after allocation",
        "arm64 object emit",
    ] {
        assert!(log.contains(step), "no `{step}` in:\n{log}");
    }
}
