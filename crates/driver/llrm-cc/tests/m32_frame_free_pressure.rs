//! m32: the pre-RA scheduler's register pressure limits are those of a function
//! with no frame pointer (`computePSetLimit`: `EBP` and `RBP` allocatable).

#[path = "common/target_dir.rs"]
mod target_dir;

use std::path::Path;
use std::process::Command;

fn compiled(level: &str) -> String {
    static CALLS: std::sync::atomic::AtomicU32 = std::sync::atomic::AtomicU32::new(0);
    let call = CALLS.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
    let dir = Path::new(env!("CARGO_TARGET_TMPDIR")).join(format!("m32fp-{level}-{}-{call}", std::process::id()));
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
        .arg(Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/c/m32_frame_free_pressure.c"))
        .output()
        .unwrap();
    assert!(done.status.success(), "{}", String::from_utf8_lossy(&done.stderr));
    std::fs::read_to_string(dir.join("d.s")).unwrap()
}

/// The limit of the GR16 pressure set was 10 (a frame pointer's) where the pin
/// has 14 without one, so a handler of 6 live values read as over the limit
/// and the scheduler hoisted the frame stores above the `ip` arithmetic that
/// feeds them (`mov byte ptr [r], 0` before `add r, ecx`), growing the live
/// ranges. In the interpreter loop of QuickrBasic's `exec_loop` that gave the
/// VM pointer's interval a rival that evicted it: 244 reloads of it, +6.8%
/// instructions at -O2. The pin keeps the source order here.
#[test]
fn test_a_frame_free_handler_keeps_its_stores_below_the_pointer_arithmetic() {
    let asm = compiled("-O2");
    let start = asm.lines().position(|line| line.trim() == "run proc near").expect("the function");
    let body: Vec<&str> = asm.lines().skip(start).take_while(|line| line.trim() != "run endp").map(str::trim).collect();
    let tag = body
        .iter()
        .position(|line| line.starts_with("mov byte ptr [") && line.ends_with("], 0"))
        .expect("the store of the frame tag");
    let adds = body.iter().position(|line| line.starts_with("add ") && line.ends_with(", 7")).expect("ip + 7");
    assert!(tag > adds, "the tag store is hoisted above `ip + 7`:\n{}", body.join("\n"));
}
