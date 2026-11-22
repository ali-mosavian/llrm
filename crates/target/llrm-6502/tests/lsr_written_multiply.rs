//! A loop that writes `i * 7 + 1` and `i * 5 + 2` for a byte counter is two
//! counters stepped by 7 and 5. LSR rated the written shift-and-add chains at
//! zero, the reduction (with a base add for `i - 20`) worse, and dropped it:
//! copy_runs -O1 ran its 48-trip init loop at 170 clocks a trip, 3.5k of the
//! 13k over llvm-mos. Needs `llrm-cc` built (`LLRM_CC`).

use std::process::Command;

mod common;
use common::pipeline::llrm_cc;

fn assembly(level: &str) -> String {
    let dir = std::path::PathBuf::from(env!("CARGO_TARGET_TMPDIR")).join("lsr-written-multiply");
    std::fs::create_dir_all(&dir).unwrap();
    let src = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/data/iv_const_mul.c");
    let out = dir.join(format!("iv.{level}.s"));
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
fn a_written_constant_multiple_of_a_counter_is_a_stepped_counter() {
    let text = assembly("-O1");
    assert!(text.contains("adc #7\n") && text.contains("adc #5\n"), "no counter stepped by 7 and 5:\n{text}");
}
