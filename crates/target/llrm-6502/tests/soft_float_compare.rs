//! The hand-written f32 and f64 compares (config/6502/cmp32.s, cmp64.s) against the host's float compare.

use std::path::Path;
use std::process::Command;

fn run(script: &str) -> String {
    let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../../runtime/compiler-rt/tests/asm6502");
    let done = Command::new("sh").arg(root.join(script)).env("EDGE_ONLY", "1").output().expect("sh");
    let text = String::from_utf8_lossy(&done.stdout).into_owned() + &String::from_utf8_lossy(&done.stderr);
    assert!(done.status.success(), "{script}:\n{text}");
    text
}

/// fcmp_all -O2 spent 478 clocks a call in the C `__ledf2` and 265 in `__lesf2`
/// (64 calls each of twelve entries, 285K of 898K clocks, 2026-11-12): every
/// pair of special values (zeros of both signs, infinities, NaNs, denormals, a
/// one-byte difference at each byte) gives the compiler-rt result, with
/// unordered 1 for eq/lt/le and -1 for ge/gt.
#[test]
fn the_f64_compares_agree_with_the_host() {
    assert!(run("run_cmp64.sh").contains("fails=0"));
}

#[test]
fn the_f32_compares_agree_with_the_host() {
    assert!(run("run_cmp32.sh").contains("fails=0"));
}
