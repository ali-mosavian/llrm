//! A constant a loop stores on every trip is made once before it
//! (`MachineLICM`): `fill`'s `lda #255` sat in the loop (14 clocks a trip
//! against 12, `long_double_constant_store`). Needs `llrm-cc` built
//! (`LLRM_CC`).

use std::process::Command;

mod common;
use common::pipeline::llrm_cc;

fn assembly(level: &str) -> String {
    let dir = std::path::PathBuf::from(env!("CARGO_TARGET_TMPDIR")).join("licm-constant");
    std::fs::create_dir_all(&dir).unwrap();
    let src = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/data/fill_counter.c");
    let out = dir.join(format!("fill.{level}.s"));
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
fn a_constant_stored_on_every_trip_is_loaded_before_the_loop() {
    for level in ["-O1", "-O2", "-Os"] {
        let text = assembly(level);
        let body = &text[text.find("\nfill:").expect("fill")..];
        let (before, rest) = body.split_once("\nfill_b2:").expect("the loop");
        let in_loop = rest.split("\nfill_b3:").next().unwrap();
        assert!(before.contains("lda #255"), "{level}: the constant is not made before the loop:\n{body}");
        assert!(!in_loop.contains("lda #255"), "{level}: the loop makes the constant every trip:\n{body}");
    }
}
