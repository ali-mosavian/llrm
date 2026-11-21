//! A void function ending in a call whose result it ignores. The pin's
//! `isEligibleForSiblingCallOpt` compares the callee's result under both
//! conventions, not against the caller's own (void) one, so a pointer result in
//! eax is a `jmp` (QuickrBasic's file_lock was `sub esp,12; call; add esp,12`),
//! and a result on the x87 stack is no sibling call at all: it needs its pop
//! (ours jumped, leaving st(0) full, one slot per call). Needs `LLRM_LLC`.

use std::path::Path;
use std::process::Command;

fn listing() -> String {
    let llc = std::env::var_os("LLRM_LLC").expect("LLRM_LLC is not set: this test needs it and does not skip");
    let dir = Path::new(env!("CARGO_TARGET_TMPDIR")).join(format!("x86-sibcall-ignored-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).unwrap();
    let object = dir.join("a.o");
    let run = Command::new(env!("CARGO_BIN_EXE_llrm-cc-bin"))
        .env("LLRM_CC_TARGET", "x86_32-linux")
        .env("LLRM_LLC", llc)
        .args(["-march=i486", "-Os", "-w", "-ffreestanding", "-c", "-o"])
        .arg(&object)
        .arg(Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/c/m32_sibcall_ignored_result.c"))
        .output()
        .unwrap();
    assert!(run.status.success(), "{}", String::from_utf8_lossy(&run.stderr));
    let dump = Command::new("objdump").args(["-d", "--no-show-raw-insn", "-M", "intel"]).arg(&object).output().unwrap();
    String::from_utf8_lossy(&dump.stdout).into_owned()
}

/// The instructions of `name`, one per line.
fn body(
    listing: &str,
    name: &str,
) -> Vec<String> {
    let head = format!("<{name}>:");
    listing
        .lines()
        .skip_while(|line| !line.contains(&head))
        .skip(1)
        .take_while(|line| !line.is_empty())
        .map(|line| line.split('\t').nth(1).unwrap_or("").trim().to_string())
        .collect()
}

#[test]
fn an_ignored_pointer_result_is_still_a_sibling_call() {
    let listing = listing();
    let ignored = body(&listing, "ignore_find");
    assert!(
        ignored.iter().any(|one| one.starts_with("jmp")) && !ignored.iter().any(|one| one.starts_with("call")),
        "ignore_find is not a jump to find:\n{}",
        ignored.join("\n")
    );
}

#[test]
fn an_ignored_x87_result_is_popped_not_jumped_over() {
    let listing = listing();
    let ignored = body(&listing, "ignore_half");
    assert!(
        ignored.iter().any(|one| one.starts_with("fstp")) && !ignored.iter().any(|one| one.starts_with("jmp")),
        "ignore_half leaves the callee's result on the x87 stack:\n{}",
        ignored.join("\n")
    );
}
