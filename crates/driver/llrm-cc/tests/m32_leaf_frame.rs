//! m32 leaf frames reserve their locals only: the omitted frame register has no
//! cell. A 4-byte local cost `sub esp, 8`; the pin's `push eax` / `pop ecx` is
//! 4 B shorter (float_return), 2026-11-08.

#[path = "common/target_dir.rs"]
mod target_dir;

use std::path::Path;
use std::process::Command;

fn compiled(level: &str) -> String {
    compiled_for(level, "x86_32-linux", &["-mabi=sysv"])
}

fn compiled_for(level: &str, row: &str, abi: &[&str]) -> String {
    // one directory per call: the tests run in parallel and shared `d.s` raced
    static CALLS: std::sync::atomic::AtomicU32 = std::sync::atomic::AtomicU32::new(0);
    let call = CALLS.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
    let dir = Path::new(env!("CARGO_TARGET_TMPDIR")).join(format!("m32frame-{level}-{}-{call}", std::process::id()));
    let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../..");
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).unwrap();
    let done = Command::new(env!("CARGO_BIN_EXE_llrm-cc-bin"))
        .env("LLRM_CC_TARGET", row)
        .env(
            "LLRM_LLC",
            std::env::var_os("LLRM_LLC").unwrap_or_else(|| target_dir::of(&root).join("release/llrm-llc").into()),
        )
        .args([level, "-w", "-S", "-march=i486"])
        .args(abi)
        .arg("-o")
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
    // Watcom rows spell a symbol with a trailing underscore
    let spelled = |line: &str, tail: &str| [name.to_owned(), format!("{name}_")].iter().any(|one| line.trim() == format!("{one} {tail}"));
    let start = asm.lines().position(|line| spelled(line, "proc near")).expect("the function");
    asm.lines().skip(start).take_while(|line| !spelled(line, "endp")).map(str::trim).collect()
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

/// A 486 pops in 4 clocks: `push eax` / `pop ecx` only where the 4 B matter (-Os); `sub esp, 4` / `add esp, 4` elsewhere
/// (FRAMESLOT-PRICE, tests/check/c/m32_one_slot_frame_by_price.c).
#[test]
fn test_a_one_slot_leaf_frame_is_push_and_pop_where_the_bytes_matter() {
    for level in ["-O1", "-O2", "-Os"] {
        let asm = compiled(level);
        for (name, pops) in [("one", "pop ecx"), ("narrow", "pop eax"), ("none", "pop eax")] {
            let body = function(&asm, name);
            // the pop is the pin's first dead caller-saved register (findDeadCallerSavedReg)
            let (open, close) = if level == "-Os" { ("push eax", pops) } else { ("sub esp, 4", "add esp, 4") };
            assert!(
                body.contains(&open) && body.contains(&close),
                "{level} {name}: expected {open} / {close}:\n{}",
                body.join("\n")
            );
        }
    }
}

/// bq32 -O1+ printed "Out of DATA" for a READ loop of 60 items: the runtime's `B$RDI2` ended its one-slot frame with `pop ecx`, which
/// loaded ECX with the pushed argument, and the caller (watcall32 keeps ecx) held its loop counter there. A convention that
/// preserves ecx gets `add esp, 4`.
#[test]
fn test_a_one_slot_frame_does_not_pop_into_a_register_the_convention_keeps() {
    for level in ["-O1", "-O2", "-Os"] {
        let asm = compiled_for(level, "x86_32-wat", &[]);
        let body = function(&asm, "one");
        assert!(
            !body.contains(&"pop ecx"),
            "{level} one: a watcall32 function must leave ecx as it found it:\n{}",
            body.join("\n")
        );
    }
}
