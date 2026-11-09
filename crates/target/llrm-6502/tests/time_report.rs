//! `LLRM_DEBUG=time` wrote nothing for the 6502: its llc never started the clock or wrote the report, and the backend
//! had no span, so a 20 s compile (`memory_expansion` -O2) could not be split into select, allocate and emit.

mod common;

use std::process::Command;

#[test]
fn the_time_report_names_the_backend_steps() {
    let dir = std::env::temp_dir().join("llrm-time-report-6502");
    std::fs::create_dir_all(&dir).unwrap();
    let source = dir.join("t.ll");
    std::fs::write(
        &source,
        format!(
            "target datalayout = \"{}\"\ndefine i16 @t(i16 %a, i16 %b) {{\n  %r = add i16 %a, %b\n  ret i16 %r\n}}\n",
            common::pipeline::datalayout()
        ),
    )
    .unwrap();
    let done = Command::new(env!("CARGO_BIN_EXE_llrm-6502-llc"))
        .env("LLRM_DEBUG", "time")
        .args(["-S", "-o", dir.join("t.s").to_str().unwrap(), source.to_str().unwrap()])
        .output()
        .expect("the driver runs");
    let stderr = String::from_utf8_lossy(&done.stderr);
    assert_eq!(done.status.code(), Some(0), "{stderr}");
    assert!(stderr.contains("[time]"), "no time report:\n{stderr}");
    for step in ["6502 select", "6502 allocate and emit", "6502 module text"] {
        assert!(stderr.contains(step), "no span `{step}`:\n{stderr}");
    }
}
