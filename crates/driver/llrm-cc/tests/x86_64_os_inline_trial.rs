//! `get` (a compare, a store, a load through a table) passes the pin's
//! `OptSizeThreshold` but the -Os size guard keeps the call (OS-INLINE-TRIAL-X86):
//! the object written for the hot loop's two calls had both, 8 instructions
//! a pass more than the finished copy. The finished object decides which sites
//! are inlined. Needs `LLRM_LLC`.

use std::path::Path;
use std::process::Command;

fn listing() -> String {
    let llc = std::env::var_os("LLRM_LLC").expect("LLRM_LLC is not set: this test needs it and does not skip");
    let dir = Path::new(env!("CARGO_TARGET_TMPDIR")).join(format!("x86-inline-trial-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).unwrap();
    let object = dir.join("a.o");
    let run = Command::new(env!("CARGO_BIN_EXE_llrm-cc-bin"))
        .env("LLRM_CC_TARGET", "x86_64-linux")
        .env("LLRM_LLC", llc)
        .args(["-Os", "-w", "-ffreestanding", "-fno-pic", "-c", "-o"])
        .arg(&object)
        .arg(Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/c/m64_os_inline_refused.c"))
        .output()
        .unwrap();
    assert!(run.status.success(), "{}", String::from_utf8_lossy(&run.stderr));
    let dump = Command::new("objdump").args(["-d", "--no-show-raw-insn", "-M", "intel"]).arg(&object).output().unwrap();
    String::from_utf8_lossy(&dump.stdout).into_owned()
}

/// The calls of `get` inside the function `name`.
fn calls_in(
    text: &str,
    name: &str,
) -> usize {
    let (_, rest) = text.split_once(&format!("<{name}>:")).expect("the function");
    let body = rest.split("\n\n").next().unwrap();
    body.lines().filter(|line| line.contains("call") && line.contains("<get>")).count()
}

#[test]
fn a_refused_callee_on_a_hot_loop_is_inlined_when_the_finished_object_is_cheaper() {
    let text = listing();
    assert_eq!(calls_in(&text, "f"), 0, "the loop's two calls cost more clocks than their copies:\n{text}");
}

#[test]
fn a_refused_callee_run_once_stays_a_call() {
    let text = listing();
    assert_eq!(calls_in(&text, "g"), 2, "run once, the copies only add bytes:\n{text}");
}
