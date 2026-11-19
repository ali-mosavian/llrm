//! m32 -O1 (INL-O1): a callee that reads a caller's local only through SROA'able
//! loads costs the caller's stores nothing once inlined, so its growth is the
//! callee's size less those stores. `kernel(&l)` of frame_address_stack_args
//! was kept as a call (+8 B counted against a 2-clock call), `main` stayed 130 B
//! against the pin's 53 B and the row ran +93 B over clang (2026-11-08). The
//! stores of a callee inlined before it (`terms(&l)`) count too: the sweep
//! decides the sites in order and the later one sees the earlier planned.

#[path = "common/target_dir.rs"]
mod target_dir;

use std::path::Path;
use std::process::Command;

#[test]
fn test_o1_inlines_a_callee_whose_local_dies_with_the_call() {
    let dir = Path::new(env!("CARGO_TARGET_TMPDIR")).join(format!("srastores-{}", std::process::id()));
    let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../..");
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).unwrap();
    let source = root.join("tests/run/c/frame_address_stack_args.c");
    let done = Command::new(env!("CARGO_BIN_EXE_llrm-cc-bin"))
        .env("LLRM_CC_TARGET", "x86_32-linux")
        .env(
            "LLRM_LLC",
            std::env::var_os("LLRM_LLC").unwrap_or_else(|| target_dir::of(&root).join("release/llrm-llc").into()),
        )
        .args(["-O1", "-w", "-S", "-march=i486", "-mabi=sysv", "-o"])
        .arg(dir.join("d.s"))
        .arg(&source)
        .output()
        .unwrap();
    assert!(done.status.success(), "{}", String::from_utf8_lossy(&done.stderr));
    let asm = std::fs::read_to_string(dir.join("d.s")).unwrap();
    // every function, `line` too: the pin leaves only `report` calls
    let calls: Vec<&str> = asm.lines().filter(|line| line.trim_start().starts_with("call")).collect();
    assert!(
        !calls.is_empty() && calls.iter().all(|line| line.trim_end().ends_with("report")),
        "a callee with a local that dies with the call was kept as a call:\n{}",
        calls.join("\n")
    );
}
