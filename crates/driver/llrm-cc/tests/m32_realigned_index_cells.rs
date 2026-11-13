//! m32: a strength-reduced index into an over-aligned local of a realigned
//! frame is addressed from the stack pointer. It stayed `[ebp + esi]`, esi from
//! -32: the array overlapped the saved registers and lay off its 16-byte
//! alignment (over_aligned_local_after_even_pushes -O1 printed 5 for 250),
//! 2026-11-09.

#[path = "common/target_dir.rs"]
mod target_dir;

use std::path::Path;
use std::process::Command;

fn compiled(level: &str) -> String {
    // one directory per call: the tests run in parallel and shared `d.s` raced
    static CALLS: std::sync::atomic::AtomicU32 = std::sync::atomic::AtomicU32::new(0);
    let call = CALLS.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
    let dir = Path::new(env!("CARGO_TARGET_TMPDIR")).join(format!("m32realign-{level}-{}-{call}", std::process::id()));
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
        .arg(Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/c/m32_realigned_index_cells.c"))
        .output()
        .unwrap();
    assert!(done.status.success(), "{}", String::from_utf8_lossy(&done.stderr));
    std::fs::read_to_string(dir.join("d.s")).unwrap()
}

fn function<'a>(
    asm: &'a str,
    name: &str,
) -> Vec<&'a str> {
    let start = asm.lines().position(|line| line.trim() == format!("{name} proc near")).expect("the function");
    asm.lines().skip(start).take_while(|line| line.trim() != format!("{name} endp")).map(str::trim).collect()
}

#[test]
fn test_a_realigned_frames_indexed_local_is_not_addressed_from_the_frame_register() {
    for level in ["-O1"] {
        let asm = compiled(level);
        let run = function(&asm, "run");
        assert!(run.iter().any(|line| line.starts_with("and esp, -16")), "{level}: not realigned:\n{}", run.join("\n"));
        let bad: Vec<&&str> = run.iter().filter(|line| line.contains("[ebp+e")).collect();
        assert!(bad.is_empty(), "{level}: locals through the frame register: {bad:?}\n{}", run.join("\n"));
    }
}
