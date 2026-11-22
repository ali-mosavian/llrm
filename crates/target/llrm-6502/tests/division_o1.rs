//! At -O1 the loop's division is a call in a recursion the mid-end does not
//! make a loop, which the static run estimate weighs at 200 runs of its 2,800:
//! the share of the module's clocks the speed routine saves read 13% against
//! the 33% of bytes it adds, and idiv_loop_os kept the compact routine, 2.34M
//! clocks against llvm-mos's 1.07M. The speed levels take the speed routine
//! wherever it repays its bytes (`[share.speed] division16 = 0`). Needs
//! `llrm-cc` built (`LLRM_CC`).

use std::process::Command;

mod common;
use common::pipeline::llrm_cc;

fn assembly(level: &str) -> String {
    let dir = std::path::PathBuf::from(env!("CARGO_TARGET_TMPDIR")).join("division-o1");
    std::fs::create_dir_all(&dir).unwrap();
    let src = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/data/gcd_sum.c");
    let out = dir.join(format!("gcd_sum.{level}.s"));
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
fn a_hot_remainder_recursion_calls_the_speed_routine_at_o1() {
    let text = assembly("-O1");
    assert!(text.contains("jsr __modhi3\n") && !text.contains("__modhi3c"), "the speed remainder at -O1:\n{text}");
}
