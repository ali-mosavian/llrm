//! x86-32 -O1 aligns no loop (DEVIATE ALIGN-O1, gcc's `-falign-loops` is -O2)
//! where SPEED-O1's byte price is on: the speed rate turned it on and queens
//! -O1 ran 3,561 clk slower on the padding of `safe`'s loop. Needs the Open
//! Watcom tree (tools/ow-fetch.sh) and `LLRM_LLC`.

use std::path::Path;
use std::process::Command;

fn compiled(level: &str) -> String {
    assert!(llrm_cc::ow_present(), "the Open Watcom tree is missing: run tools/ow-fetch.sh; this test does not skip");
    let llc = std::env::var_os("LLRM_LLC").expect("LLRM_LLC is not set: this test needs it and does not skip");
    let dir = Path::new(env!("CARGO_TARGET_TMPDIR")).join(format!("m32rec-{level}-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).unwrap();
    let done = Command::new(env!("CARGO_BIN_EXE_llrm-cc-bin"))
        .env("LLRM_CC_TARGET", "x86_32-wat")
        .env("LLRM_LLC", llc)
        .args([level, "-w", "-S", "-march=i486", "-o"])
        .arg(dir.join("d.s"))
        .arg(Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/c/o1_loop_alignment_queens.c"))
        .output()
        .unwrap();
    assert!(done.status.success(), "{}", String::from_utf8_lossy(&done.stderr));
    std::fs::read_to_string(dir.join("d.s")).unwrap()
}

#[test]
fn test_o1_does_not_align_loops() {
    let asm = compiled("-O1");
    assert!(!asm.lines().any(|line| line.trim().starts_with("align")), "-O1 aligned a loop:\n{asm}");
}
