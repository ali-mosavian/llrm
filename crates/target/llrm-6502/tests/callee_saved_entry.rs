//! A callee-saved cell is pushed before an argument that arrives in A is copied into it.
//! Needs `llrm-cc` built (`LLRM_CC`).

use std::process::Command;

mod common;
use common::pipeline::llrm_cc;

fn assembly(source: &str) -> String {
    let dir = std::path::PathBuf::from(env!("CARGO_TARGET_TMPDIR")).join("callee-saved-entry");
    std::fs::create_dir_all(&dir).unwrap();
    let src = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/data").join(source);
    let out = dir.join(format!("{source}.s"));
    let made = Command::new(llrm_cc())
        .current_dir(&dir)
        .args(["--target=6502-sim65", "-O2", "-S", "-w", "-o"])
        .arg(&out)
        .arg(&src)
        .output()
        .unwrap();
    assert!(made.status.success(), "{}", String::from_utf8_lossy(&made.stderr));
    std::fs::read_to_string(out).unwrap()
}

/// `mix(long, long, long)` copied its first byte from A into `rc8` and then pushed A as "the old
/// `rc8`": the caller's `rc8` was lost and `frameless` -O2 printed 1011 for 499 (2026-11-14). The
/// fixture has no multiply: the shape (an argument in A copied into a callee-saved cell) must not
/// depend on how a multiply is called.
#[test]
fn a_cell_an_argument_is_copied_into_is_pushed_before_the_copy() {
    let text = assembly("saved_cell_arg.c");
    let body = &text[text.find("\nmix:").expect("mix")..];
    let body = body.split(".segment").next().unwrap();
    let store = body.find("  sta rc8\n").unwrap_or_else(|| panic!("no copy into rc8:\n{body}"));
    let before = &body[..store];
    assert!(before.contains("  lda rc8\n  pha\n"), "rc8 is overwritten before its old value is pushed:\n{body}");
    // the argument waits in an index register while the cells are pushed, and comes back for the copy
    let held = ["  tay\n", "  tax\n"].iter().zip(["  tya\n", "  txa\n"]).any(|(save, back)| {
        before.find(save).zip(before.find("  lda rc8\n")).is_some_and(|(at, push)| at < push) && before.ends_with(back)
    });
    assert!(held, "the argument in A is lost across the pushes:\n{body}");
}
