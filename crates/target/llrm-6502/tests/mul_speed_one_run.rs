//! A module whose one 32-bit multiply runs once linked the compact `__mulsi3`,
//! which walks all 32 bits of the multiplier: uint64_to_float -O2 ran 2,685
//! clocks against llvm-mos's 1,977 (the same call, its walk stops at the top
//! set bit). At speed a run saves more than the routine's bytes cost at the
//! rate of every speed form (12 clocks a byte). Needs `llrm-cc` built
//! (`LLRM_CC`).

use std::process::Command;

mod common;
use common::pipeline::llrm_cc;

fn assembly(level: &str) -> String {
    let dir = std::path::PathBuf::from(env!("CARGO_TARGET_TMPDIR")).join("mul-speed-one-run");
    std::fs::create_dir_all(&dir).unwrap();
    let src = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/data/mul_once.c");
    let out = dir.join(format!("mul.{level}.s"));
    let made = Command::new(llrm_cc())
        .current_dir(&dir)
        .args(["--target=6502-sim65", level, "-S", "-w", "-fsigned-char", "-fwhole-program", "-o"])
        .arg(&out)
        .arg(&src)
        .output()
        .unwrap();
    assert!(made.status.success(), "{}", String::from_utf8_lossy(&made.stderr));
    std::fs::read_to_string(out).unwrap()
}

#[test]
fn one_multiply_at_speed_calls_the_early_exit_routine() {
    for level in ["-O1", "-O2", "-O3"] {
        let text = assembly(level);
        assert!(text.contains("jsr __mulsi3_fast\n"), "{level}: no speed multiply:\n{text}");
    }
}

#[test]
fn one_multiply_at_size_keeps_the_compact_routine() {
    let text = assembly("-Os");
    assert!(text.contains("jsr __mulsi3\n") && !text.contains("__mulsi3_fast"), "-Os:\n{text}");
}
