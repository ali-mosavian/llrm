//! x86-32 -O1 keeps to what gcc's -O1 does where SPEED-O1's byte price is on:
//! no recursion copied into itself (gcc's is -O2, the pin's inliner never) and
//! no loop alignment (DEVIATE ALIGN-O1). The speed rate turned both on: queens
//! -O1 ran 164,704 clk and 559 B against 161,143 and 183 B. Needs the Open
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
        .arg(Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/c/o1_recursion_queens.c"))
        .output()
        .unwrap();
    assert!(done.status.success(), "{}", String::from_utf8_lossy(&done.stderr));
    std::fs::read_to_string(dir.join("d.s")).unwrap()
}

fn calls_of_place(asm: &str) -> usize {
    asm.lines().filter(|line| line.trim() == "call place_").count()
}

#[test]
fn test_o1_does_not_copy_a_recursion_into_itself() {
    let asm = compiled("-O1");
    // the call from `bench_queens` and the recursion's own: the copies made
    // nine
    assert!(calls_of_place(&asm) <= 3, "-O1 copied `place` into itself:\n{asm}");
}

#[test]
fn test_o1_does_not_align_loops() {
    let asm = compiled("-O1");
    assert!(!asm.lines().any(|line| line.trim().starts_with("align")), "-O1 aligned a loop:\n{asm}");
}
