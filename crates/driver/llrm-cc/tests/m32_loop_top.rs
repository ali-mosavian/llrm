//! m32 (i486): the latch of an outer loop stays below the inner loop it falls
//! from (DEVIATE LOOP-TOP-JMP), as gcc's `bb-reorder` lays it out.

#[path = "common/target_dir.rs"]
mod target_dir;

use std::path::Path;
use std::process::Command;

fn compiled(level: &str) -> String {
    static CALLS: std::sync::atomic::AtomicU32 = std::sync::atomic::AtomicU32::new(0);
    let call = CALLS.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
    let dir = Path::new(env!("CARGO_TARGET_TMPDIR")).join(format!("m32top-{level}-{}-{call}", std::process::id()));
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
        .arg(Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/c/m32_loop_top.c"))
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

/// The guard `n <= 0` is estimated at 12 in 32, so the latch was placed above
/// the loop's header and the inner loop's exit took a `jmp` to it every outer
/// trip: 9 `jmp` and 8 `je` for +37 clk over gcc (x_ld_dot -O2).
#[test]
fn test_the_inner_loop_exit_falls_into_the_latch() {
    for level in ["-O1", "-O2", "-O3"] {
        let asm = compiled(level);
        let body = function(&asm, "dots");
        let first = body.iter().position(|line| line.starts_with("jne ")).expect("the inner loop's branch");
        assert!(
            !body[first + 1].starts_with("jmp "),
            "{level}: the inner loop's exit jumps to its latch:\n{}",
            body.join("\n")
        );
    }
}

/// An inner loop of two arms that join in the latch: the latch, copied into
/// both arms, goes above the header, so the exit falls into the outer latch.
/// With the latch below, both arms ended `jne header; jmp exit` (x_cordic
/// -O1/-O2: 50,653 clk against 48,939).
#[test]
fn test_a_two_arm_inner_loop_exit_is_not_a_jump() {
    for level in ["-O1", "-O2"] {
        let asm = compiled(level);
        let body = function(&asm, "cordic");
        let jumped = body.windows(2).any(|pair| pair[0].starts_with("jne ") && pair[1].starts_with("jmp "));
        assert!(!jumped, "{level}: an arm's exit jumps after its `jne`:\n{}", body.join("\n"));
    }
}
