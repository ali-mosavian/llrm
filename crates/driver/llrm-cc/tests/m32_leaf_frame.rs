//! m32 leaf frames reserve their locals only: the omitted frame register has no
//! cell. A 4-byte local cost `sub esp, 8`; the pin's `push eax` / `pop ecx` is
//! 4 B shorter (float_return), 2026-11-08.

#[path = "common/target_dir.rs"]
mod target_dir;

use std::path::Path;
use std::process::Command;

fn compiled(level: &str) -> String {
    // one directory per call: the tests run in parallel and shared `d.s` raced
    static CALLS: std::sync::atomic::AtomicU32 = std::sync::atomic::AtomicU32::new(0);
    let call = CALLS.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
    let dir = Path::new(env!("CARGO_TARGET_TMPDIR")).join(format!("m32frame-{level}-{}-{call}", std::process::id()));
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
        .arg(Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/c/m32_leaf_frame.c"))
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
fn test_a_leaf_frame_has_no_cell_for_the_omitted_frame_register() {
    for level in ["-O1", "-O2", "-Os"] {
        let asm = compiled(level);
        for (name, size) in [("two", 8)] {
            let body = function(&asm, name);
            assert!(
                body.iter().any(|line| *line == format!("sub esp, {size}")),
                "{level} {name}: expected sub esp, {size}:\n{}",
                body.join("\n")
            );
        }
    }
}

#[test]
fn test_a_one_slot_leaf_frame_is_push_and_pop() {
    for level in ["-O1", "-O2", "-Os"] {
        let asm = compiled(level);
        for name in ["one", "narrow"] {
            let body = function(&asm, name);
            assert!(
                body.contains(&"push eax") && body.contains(&"pop ecx"),
                "{level} {name}: expected push eax / pop ecx:\n{}",
                body.join("\n")
            );
        }
    }
}
