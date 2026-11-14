//! A loop the late function round empties is deleted by the LoopDeletion that follows its loop rotate
//! (PassBuilderPipelines.cpp:1628). Without that slot m32 -O3 kept `mov eax, edi; dec eax; jne` for a
//! byte copied onto itself (memfuncs_runtime, M32-TIP-3, +6 B), 2026-11-11.

#[path = "common/target_dir.rs"]
mod target_dir;

use std::path::Path;
use std::process::Command;

fn compiled() -> String {
    let dir = Path::new(env!("CARGO_TARGET_TMPDIR")).join(format!("late-loop-deletion-{}", std::process::id()));
    let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../..");
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).unwrap();
    let done = Command::new(env!("CARGO_BIN_EXE_llrm-cc-bin"))
        .env("LLRM_CC_TARGET", "x86_32-linux")
        .env(
            "LLRM_LLC",
            std::env::var_os("LLRM_LLC").unwrap_or_else(|| target_dir::of(&root).join("release/llrm-llc").into()),
        )
        .args(["-O3", "-w", "-S", "-march=i486", "-mabi=sysv", "-o"])
        .arg(dir.join("d.s"))
        .arg(Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/c/late_loop_deletion.c"))
        .output()
        .unwrap();
    assert!(done.status.success(), "{}", String::from_utf8_lossy(&done.stderr));
    std::fs::read_to_string(dir.join("d.s")).unwrap()
}

/// A label whose next two lines are `dec REG` and `jne` back to it: a loop with nothing in it.
fn empty_counting_loops(asm: &str) -> Vec<String> {
    let lines: Vec<&str> = asm.lines().map(str::trim).collect();
    lines
        .windows(3)
        .filter_map(|w| {
            let label = w[0].strip_suffix(':')?;
            (w[1].starts_with("dec e") && w[2] == format!("jne {label}")).then(|| label.to_owned())
        })
        .collect()
}

#[test]
fn test_a_loop_the_late_round_emptied_is_deleted() {
    let asm = compiled();
    let empty = empty_counting_loops(&asm);
    assert!(empty.is_empty(), "empty counting loops {empty:?}:\n{asm}");
}
