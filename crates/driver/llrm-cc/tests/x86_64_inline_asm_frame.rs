//! x86-64 -O2 and -Os: an inline `cvtss2si` (register operands only) does not
//! keep the frame register. Any inline code did, so `push rbp; mov rbp, rsp`
//! stood in a function with no frame object of its own and the allocator lost
//! `rbp` (QB `exec_loop`, three `pfpu_rint32`: 22,365 B against 22,253 once it
//! is free). At -Os only m32 freed it: QB m64 -Os `exec_loop` lost `rbp` and
//! kept 161 table `lea`s once its dispatch was copied (16,549 B, 16,246 with
//! `rbp` free). Needs `LLRM_LLC`.

use std::path::Path;
use std::process::Command;

fn assembly(level: &str) -> String {
    let llc = std::env::var_os("LLRM_LLC").expect("LLRM_LLC is not set: this test needs it and does not skip");
    let dir = Path::new(env!("CARGO_TARGET_TMPDIR")).join(format!("ia-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).unwrap();
    let asm = dir.join("a.s");
    let run = Command::new(env!("CARGO_BIN_EXE_llrm-cc-bin"))
        .env("LLRM_CC_TARGET", "x86_64-linux")
        .env("LLRM_LLC", llc)
        .args([level, "-w", "-fno-pic", "-S", "-o"])
        .arg(&asm)
        .arg(Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/c/x86_64_inline_asm_frame.c"))
        .output()
        .unwrap();
    assert!(run.status.success(), "{}", String::from_utf8_lossy(&run.stderr));
    std::fs::read_to_string(&asm).unwrap()
}

#[test]
fn an_inline_register_only_sse_instruction_keeps_no_frame_pointer() {
    for level in ["-O2", "-Os"] {
        let text = assembly(level);
        assert!(!text.contains("mov rbp, rsp"), "{level}: frame pointer kept:\n{text}");
    }
}
