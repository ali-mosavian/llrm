//! A program that calls `__mulhi3` and `__mulsi3` at -Os prices the first
//! from its text in `mos_rt.s`, which runs to the next `.export`. The word
//! divide's text was moved under an `.export` of its own and `__mulhi3` lost
//! the 25 lines of `udivmod16` it was priced with: phi_slot_unknown_arm then
//! took `__mulsi3_fast` at -Os, 19 B more (1044 -> 1063 B, 2026-11-15). The
//! speed routine stops its walk at the top set bit and is 9 B over the compact
//! one, so phi_slot_unknown_arm's loop pays for it: smaller (959 -> 949 B) and
//! 20% fewer clocks. The price still must come from the routine's own text.

mod common;

use std::process::Command;

#[test]
fn a_loop_multiply_takes_the_speed_routine_at_size_by_its_own_price() {
    let dir = std::path::PathBuf::from(env!("CARGO_TARGET_TMPDIR")).join("multiply-price-stable");
    std::fs::create_dir_all(&dir).unwrap();
    let src = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../../tests/run/c/phi_slot_unknown_arm.c");
    let made = Command::new(common::pipeline::llrm_cc())
        .current_dir(&dir)
        .args(["--target=6502-sim65", "-Os", "-S", "-w", "-fwhole-program", "-o", "o.s"])
        .arg(&src)
        .output()
        .unwrap();
    assert!(made.status.success(), "{}", String::from_utf8_lossy(&made.stderr));
    let text = std::fs::read_to_string(dir.join("o.s")).unwrap();
    assert!(text.contains("__mulsi3_fast"), "the compact multiply at -Os:\n{text}");
}
