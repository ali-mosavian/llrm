//! x86-32 -Os: a call's one or two argument words come off the stack by
//! `pop` (1 byte each), not `add esp,N` (3). QuickrBasic's `ctrl_stmt` carried
//! 108 of them: m32 Os was +21 B over the best of gcc and clang. Needs
//! `LLRM_LLC`.

use std::path::Path;
use std::process::Command;

#[test]
fn a_call_pops_its_one_or_two_argument_words_at_os() {
    let llc = std::env::var_os("LLRM_LLC").expect("LLRM_LLC is not set: this test needs it and does not skip");
    let dir = Path::new(env!("CARGO_TARGET_TMPDIR")).join(format!("os-pop-args-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).unwrap();
    let asm = dir.join("a.s");
    let run = Command::new(env!("CARGO_BIN_EXE_llrm-cc-bin"))
        .env("LLRM_CC_TARGET", "x86_32-linux")
        .env("LLRM_LLC", llc)
        .args(["-Os", "-w", "-S", "-o"])
        .arg(&asm)
        .arg(Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/c/pop_args.c"))
        .output()
        .unwrap();
    assert!(run.status.success(), "{}", String::from_utf8_lossy(&run.stderr));
    let text = std::fs::read_to_string(&asm).unwrap();
    assert!(!text.contains("add esp, 4") && !text.contains("add esp, 8"), "a call cleaned up by add:\n{text}");
    assert!(text.contains("pop "), "no pop cleanup:\n{text}");
}
