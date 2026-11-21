//! A long modulus in a counted loop calls the wide speed path at -Os too, as at
//! -O2. -Os linked the small wide path whatever the trips: copy_runs ran
//! 123,790 clocks against 60,901 at -O2 (llvm-mos 48,393), 423 clocks a call
//! for 141 bytes. Needs `llrm-cc` built (`LLRM_CC`).

use std::process::Command;

mod common;
use common::pipeline::llrm_cc;

fn assembly(level: &str) -> String {
    let dir = std::path::PathBuf::from(env!("CARGO_TARGET_TMPDIR")).join("os-wide-division");
    std::fs::create_dir_all(&dir).unwrap();
    let src = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/data/hot_modulus.c");
    let out = dir.join(format!("hot_modulus.{level}.s"));
    let made = Command::new(llrm_cc())
        .current_dir(&dir)
        .args(["--target=6502-sim65", level, "-fno-inline-functions", "-S", "-w", "-o"])
        .arg(&out)
        .arg(&src)
        .output()
        .unwrap();
    assert!(made.status.success(), "{}", String::from_utf8_lossy(&made.stderr));
    std::fs::read_to_string(out).unwrap()
}

#[test]
fn a_modulus_in_a_counted_loop_calls_the_wide_path_at_size() {
    for level in ["-O2", "-Os"] {
        let text = assembly(level);
        assert!(text.contains("jsr __modsi3_fast"), "{level}: no speed modulus (premise):\n{text}");
        assert!(
            !text.contains("__modsi3_fast_small"),
            "{level}: the loop's modulus calls the small wide path:\n{text}"
        );
    }
}
