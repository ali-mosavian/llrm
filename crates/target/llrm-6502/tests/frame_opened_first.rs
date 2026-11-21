//! The frame is opened (`rc0` adjusted) before a register save or a spill is
//! stored into it. A recursive function of 20 bytes of arguments stored its
//! saved `rc8`/`rc9` and a spilled argument at the offsets of the new frame
//! from the old stack pointer, into the caller's slots: `arg_registers_abi`
//! (hand-sunk `deep`) HALTed at -O1..-Os on the plain compiler. Needs
//! `llrm-cc` built (`LLRM_CC`).

use std::process::Command;

mod common;
use common::pipeline::llrm_cc;

fn assembly(level: &str) -> String {
    let dir = std::path::PathBuf::from(env!("CARGO_TARGET_TMPDIR")).join("frame-opened-first");
    std::fs::create_dir_all(&dir).unwrap();
    let src = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/data/recursive_stack_args.c");
    let out = dir.join(format!("recursive_stack_args.{level}.s"));
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
fn nothing_is_stored_through_the_stack_pointer_before_the_frame_is_opened() {
    for level in ["-O1", "-O2", "-Os"] {
        let text = assembly(level);
        let body = &text[text.find("\ndeep:").expect("deep")..];
        let body = body.split("\ndeep_b1:").next().unwrap();
        let opened = body.find("  sta rc0\n").or_else(|| body.find("  jsr __outl_")).expect("no frame adjust (premise)");
        let first = body.find("(rc0),y").expect("no frame access (premise)");
        assert!(opened < first, "{level}: the frame is accessed before it is opened:\n{body}");
    }
}
