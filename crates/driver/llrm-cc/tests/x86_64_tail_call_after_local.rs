//! `tail call f(); llvm.lifetime.end(local); ret` was no tail call: the pin's
//! `isInTailCallPosition` skips `lifetime.end`, ours stopped at it, so a
//! function that fills a local and returns a call ended `call; pop; ret`
//! where the pin jumps. Needs `LLRM_LLC`.

use std::path::Path;
use std::process::Command;

#[test]
fn a_call_after_a_local_is_filled_is_a_jump() {
    let llc = std::env::var_os("LLRM_LLC").expect("LLRM_LLC is not set: this test needs it and does not skip");
    let dir = Path::new(env!("CARGO_TARGET_TMPDIR")).join(format!("tail-after-local-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).unwrap();
    let object = dir.join("a.o");
    let run = Command::new(env!("CARGO_BIN_EXE_llrm-cc-bin"))
        .env("LLRM_CC_TARGET", "x86_64-linux")
        .env("LLRM_LLC", llc)
        .args(["-O2", "-w", "-ffreestanding", "-c", "-o"])
        .arg(&object)
        .arg(Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/c/tail_after_local.c"))
        .output()
        .unwrap();
    assert!(run.status.success(), "{}", String::from_utf8_lossy(&run.stderr));
    let dump = Command::new("objdump").args(["-dr", "--no-show-raw-insn"]).arg(&object).output().unwrap();
    let text = String::from_utf8_lossy(&dump.stdout).into_owned();
    let body: Vec<_> = text
        .lines()
        .skip_while(|line| !line.contains("<local_then_tail>:"))
        .skip(1)
        .take_while(|line| !line.is_empty())
        .collect();
    let last = body.iter().rev().find(|line| line.contains(":\t")).expect("an instruction");
    assert!(last.contains("jmp"), "the last instruction jumps to finish:\n{text}");
    assert!(!body.iter().any(|line| line.contains("ret")), "no return after the call:\n{text}");
}
