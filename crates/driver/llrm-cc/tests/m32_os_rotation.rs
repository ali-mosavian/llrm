//! m32 (i486) -Os: a loop is rotated where the finished code runs less for a
//! few bytes (ROTATION-HOT), priced with its `fxch`es at their clocks. One
//! module per case: the trial keeps or puts back a module's rotations together.

#[path = "common/target_dir.rs"]
mod target_dir;

use std::path::Path;
use std::process::Command;

fn compiled(file: &str) -> String {
    static CALLS: std::sync::atomic::AtomicU32 = std::sync::atomic::AtomicU32::new(0);
    let call = CALLS.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
    let dir = Path::new(env!("CARGO_TARGET_TMPDIR")).join(format!("m32osrot-{}-{call}", std::process::id()));
    let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../..");
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).unwrap();
    let done = Command::new(env!("CARGO_BIN_EXE_llrm-cc-bin"))
        .env("LLRM_CC_TARGET", "x86_32-linux")
        .env(
            "LLRM_LLC",
            std::env::var_os("LLRM_LLC").unwrap_or_else(|| target_dir::of(&root).join("release/llrm-llc").into()),
        )
        .args(["-Os", "-w", "-S", "-march=i486", "-mabi=sysv", "-o"])
        .arg(dir.join("d.s"))
        .arg(Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/c").join(file))
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

/// A counted loop was left with its exit test at the top and a `jmp` back on
/// every trip, for 5 bytes over the 4 a loop was allowed (floats -Os: 112,090
/// clk against clang's rotated 111,087).
#[test]
fn test_a_counted_loop_is_rotated_for_a_guard_of_bytes() {
    let body = function(&compiled("m32_os_rotation_count.c"), "bench_floats");
    assert!(!body.iter().any(|line| line.starts_with("jmp ")), "a `jmp` closes the loop:\n{}", body.join("\n"));
}

/// The rotated loop of a tail recursion holds two `fxch` a trip, 4 clocks each
/// on the 486, which the work model counted as one unit (recfloat -Os: 39,091
/// clk rotated to 46,090).
#[test]
fn test_a_rotation_that_adds_fxch_is_not_kept() {
    let body = function(&compiled("m32_os_rotation_fxch.c"), "decay_all");
    let exchanges = body.iter().filter(|line| line.starts_with("fxch")).count();
    assert!(exchanges <= 1, "{exchanges} `fxch` in the loop:\n{}", body.join("\n"));
}
