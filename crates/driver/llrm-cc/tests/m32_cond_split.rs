//! m32 (i486): the float sum only the second test of `a && b` reads is made
//! after the first test (DEVIATE CB1), as gcc's `tree-ssa-sink` does.

#[path = "common/target_dir.rs"]
mod target_dir;

use std::path::Path;
use std::process::Command;

fn compiled(level: &str) -> String {
    static CALLS: std::sync::atomic::AtomicU32 = std::sync::atomic::AtomicU32::new(0);
    let call = CALLS.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
    let dir = Path::new(env!("CARGO_TARGET_TMPDIR")).join(format!("m32cond-{level}-{}-{call}", std::process::id()));
    let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../..");
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).unwrap();
    let done = Command::new(env!("CARGO_BIN_EXE_llrm-cc-bin"))
        .env("LLRM_CC_TARGET", "x86_32-linux")
        .env(
            "LLRM_LLC",
            std::env::var_os("LLRM_LLC").unwrap_or_else(|| target_dir::of(&root).join("release/llrm-llc").into()),
        )
        .args([level, "-w", "-S", "-march=i486", "-mabi=sysv", "-o"])
        .arg(dir.join("d.s"))
        .arg(Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/c/m32_cond_split.c"))
        .output()
        .unwrap();
    assert!(done.status.success(), "{}", String::from_utf8_lossy(&done.stderr));
    std::fs::read_to_string(dir.join("d.s")).unwrap()
}

fn function(
    asm: &str,
    name: &str,
) -> Vec<String> {
    let start = asm.lines().position(|line| line.trim() == format!("{name} proc near")).expect("the function");
    asm.lines()
        .skip(start)
        .take_while(|line| line.trim() != format!("{name} endp"))
        .map(|line| line.trim().to_owned())
        .collect()
}

/// The `fld`, `fadd` and `fstp` ran on every trip though the exponent test
/// decided 16 of 400: 400 `fadd` against gcc's 384 (sfm16_dbg -O1 +291 clk).
/// A size build keeps the one block.
#[test]
fn test_the_sum_is_made_after_the_test_that_may_skip_it() {
    for level in ["-O1", "-O2"] {
        let asm = compiled(level);
        let body = function(&asm, "mismatches");
        let jumps = |line: &String| line.split_whitespace().next().is_some_and(|m| m.starts_with('j') && m != "jmp");
        let first_test = body
            .iter()
            .position(|line| line.starts_with("test ") && line.contains("2139095040"))
            .expect("the exponent test");
        let first_jump = first_test + body[first_test..].iter().position(jumps).expect("its jump");
        let sum = body.iter().position(|line| line.starts_with("fadd")).expect("the sum");
        assert!(sum > first_jump, "{level}: the sum is made before the exponent test:\n{}", body.join("\n"));
    }
}
