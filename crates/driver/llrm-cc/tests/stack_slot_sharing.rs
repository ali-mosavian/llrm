//! m32 shares stack slots as the pin does (`ColorSlot`): any two whose ranges
//! do not overlap. The width rule that kept a word out of a loop-held wider
//! slot (SLOTS-LOOPWIDTH, m16 only now) grew `overflow_builtins` -Os by 23 B:
//! `sub esp, 32` against the pin's 28.

use std::path::Path;
use std::process::Command;

#[test]
fn m32_shares_a_word_slot_with_a_loop_held_wider_one_as_the_pin_does() {
    let source = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../../tests/run/c/overflow_builtins.c");
    let dir = Path::new(env!("CARGO_TARGET_TMPDIR")).join(format!("slotshare-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).unwrap();
    let run = Command::new(env!("CARGO_BIN_EXE_llrm-cc-bin"))
        .env("LLRM_CC_TARGET", "x86_32-linux")
        .args(["-Os", "-m32", "-w", "-S", "-o"])
        .arg(dir.join("o.s"))
        .arg(&source)
        .output()
        .unwrap();
    assert!(run.status.success(), "{}", String::from_utf8_lossy(&run.stderr));
    let text = std::fs::read_to_string(dir.join("o.s")).unwrap();
    assert!(text.contains("sub esp, 28"), "the pin's frame is 28 B; got:\n{text}");
}
