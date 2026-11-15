//! k_strnicmp at -O1: clang's -O1 IR leaves the `break` as a `switch` on a phi
//! of constants (pin -O1 has no JumpThreadingPass), which cost 197 B against
//! 112 B with it. Needs `llrm-cc` built (`LLRM_CC`) and cc65.

use std::process::Command;

mod common;
use common::pipeline::llrm_cc;

#[test]
fn k_strnicmp_at_o1_threads_the_loop_exit() {
    let dir = std::path::PathBuf::from(env!("CARGO_TARGET_TMPDIR")).join("strnicmp-o1");
    std::fs::create_dir_all(&dir).unwrap();
    let src = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/bench/hand/strnicmp.c");
    let made = Command::new(llrm_cc())
        .current_dir(&dir)
        .args(["--target=6502-sim65", "-O1", "-S", "-w", "-o", "o.s"])
        .arg(&src)
        .output()
        .unwrap();
    assert!(made.status.success(), "{}", String::from_utf8_lossy(&made.stderr));
    let built = Command::new("ca65").current_dir(&dir).args(["-U", "-o", "o.o", "o.s"]).output().unwrap();
    assert!(built.status.success(), "{}", String::from_utf8_lossy(&built.stderr));
    let dump = Command::new("od65").current_dir(&dir).args(["--dump-segsize", "o.o"]).output().unwrap();
    let text = String::from_utf8_lossy(&dump.stdout);
    let code: u32 = text
        .lines()
        .find_map(|l| l.trim().strip_prefix("CODE:").and_then(|n| n.trim().parse().ok()))
        .unwrap_or_else(|| panic!("no CODE size in {text}"));
    assert!(code <= 120, "CODE {code} B; unthreaded 197 B, threaded 112 B");
}
