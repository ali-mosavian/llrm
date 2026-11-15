//! A callee-saved cell nothing uses is not saved. Needs `llrm-cc` built
//! (`LLRM_CC`).

use std::process::Command;

mod common;
use common::pipeline::llrm_cc;

/// `main` of `memmove_overlap` -Os pushed and pulled `rc8` (6 B) though every
/// store into it was dead and removed after the allocator chose the cell.
#[test]
fn a_callee_saved_cell_with_no_use_is_not_pushed() {
    let dir = std::path::PathBuf::from(env!("CARGO_TARGET_TMPDIR")).join("unused-saves");
    std::fs::create_dir_all(&dir).unwrap();
    let src = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/data/unused_save.c");
    let out = dir.join("unused_save.s");
    let made = Command::new(llrm_cc())
        .current_dir(&dir)
        .args(["--target=6502-sim65", "-Os", "-fwhole-program", "-S", "-w", "-o"])
        .arg(&out)
        .arg(&src)
        .output()
        .unwrap();
    assert!(made.status.success(), "{}", String::from_utf8_lossy(&made.stderr));
    let text = std::fs::read_to_string(out).unwrap();
    let main = &text[text.find("\nmain:").expect("main")..];
    let main = main.split(".segment").next().unwrap();
    assert!(!main.contains("  lda rc8\n  pha\n"), "{main}");
}
