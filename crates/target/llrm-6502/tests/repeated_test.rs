//! A test the code just made and found false is not made again. Needs `llrm-cc`
//! built (`LLRM_CC`).

use std::process::Command;

mod common;
use common::pipeline::llrm_cc;

/// `k_memset` -O2 tested `lda rc4; ora rc5; beq` at entry and again at the top
/// of the fill loop, 6 B for a branch that cannot be taken.
#[test]
fn a_zero_test_straight_after_the_same_test_is_dropped() {
    let dir = std::path::PathBuf::from(env!("CARGO_TARGET_TMPDIR")).join("repeated-test");
    std::fs::create_dir_all(&dir).unwrap();
    let src = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/data/repeated_guard.c");
    let out = dir.join("repeated_guard.s");
    let made = Command::new(llrm_cc())
        .current_dir(&dir)
        .args(["--target=6502-sim65", "-O2", "-S", "-w", "-o"])
        .arg(&out)
        .arg(&src)
        .output()
        .unwrap();
    assert!(made.status.success(), "{}", String::from_utf8_lossy(&made.stderr));
    let text = std::fs::read_to_string(out).unwrap();
    let body = &text[text.find("\nk_memset:").expect("k_memset")..];
    let body = body.split(".segment").next().unwrap();
    assert_eq!(body.matches("  lda rc4\n  ora rc5\n  beq").count(), 1, "{body}");
}
