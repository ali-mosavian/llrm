//! The frame is opened (`rc0` adjusted) before a register save or a spill is
//! stored into it. A recursive function of 20 bytes of arguments stored its
//! saved `rc8`/`rc9` and a spilled argument at the offsets of the new frame
//! from the old stack pointer, into the caller's slots: `arg_registers_abi`
//! (hand-sunk `deep`) HALTed at -O1..-Os on the plain compiler. Needs
//! `llrm-cc` built (`LLRM_CC`). The source's result is `t * 3 + s`: with `t +
//! s` tailcallelim's accumulator (#10651) made the recursion a loop and no
//! frame was left to open.

use std::process::Command;

mod common;
use common::pipeline::llrm_cc;

fn assembly(
    level: &str,
    test: &str,
) -> String {
    let dir = std::path::PathBuf::from(env!("CARGO_TARGET_TMPDIR")).join(format!("frame-opened-first-{test}"));
    std::fs::create_dir_all(&dir).unwrap();
    let src = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/data/recursive_stack_args_frame.c");
    let out = dir.join(format!("recursive_stack_args_frame.{level}.s"));
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
        let text = assembly(level, "stores");
        let body = &text[text.find("\ndeep:").expect("deep")..];
        let body = body.split("\ndeep_b1:").next().unwrap();
        let opened =
            body.find("  sta rc0\n").or_else(|| body.find("  jsr __outl_")).expect("no frame adjust (premise)");
        let first = body.find("(rc0),y").expect("no frame access (premise)");
        assert!(opened < first, "{level}: the frame is accessed before it is opened:\n{body}");
    }
}

/// The two argument bytes that arrive on the stack are read above the whole
/// frame (the 14 bytes `adc #242` opens): the load used the frame size before
/// the register saves and spills were added, `ldy #2` read a spill slot, and
/// the result's upper three bytes were garbage (101056552 for 40).
#[test]
fn an_argument_on_the_stack_is_read_above_the_saves_and_spills() {
    for level in ["-O1", "-O2", "-Os"] {
        let text = assembly(level, "reads");
        let body = &text[text.find("\ndeep:").expect("deep")..];
        let body = body.split("\ndeep_b1:").next().unwrap();
        let (_, after) = body.split_once("adc #").expect("no frame adjust (premise)");
        let opened: u32 = 256 - after.split('\n').next().unwrap().parse::<u32>().expect("an adjust of the frame");
        assert!(
            body.contains(&format!("  ldy #{opened}\n  lda (rc0),y\n")),
            "{level}: the stack argument is not read at offset {opened}:\n{body}"
        );
    }
}
