//! A function that calls itself runs its body once per depth, as a loop does.
//! frameless -O1 ran 48,561 clocks against llvm-mos's 45,903: `descend(10)`
//! multiplied `n * 3` by a call of the 32-bit routine eleven times (the
//! multiply-call trial saw a function called once, outside every loop). Needs
//! `llrm-cc` built (`LLRM_CC`).

use std::process::Command;

mod common;
use common::pipeline::llrm_cc;

fn assembly(level: &str) -> String {
    let dir = std::path::PathBuf::from(env!("CARGO_TARGET_TMPDIR")).join("recursive-hot");
    std::fs::create_dir_all(&dir).unwrap();
    let src = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/data/recursive_mul.c");
    let out = dir.join(format!("rec.{level}.s"));
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
fn a_multiply_in_a_recursion_is_not_a_call_at_speed() {
    for level in ["-O1", "-O2", "-O3"] {
        let text = assembly(level);
        let calls = text.matches("jsr __mulsi3").count();
        assert_eq!(calls, 1, "{level}: only `scale` calls the routine:\n{text}");
    }
}
