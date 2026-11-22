//! x86-32 -Os: a call's one or two argument words come off the stack by
//! `pop` (1 byte each), not `add esp,N` (3). QuickrBasic's `ctrl_stmt` carried
//! 108 of them: m32 Os was +21 B over the best of gcc and clang. A pop is 4
//! clocks on the 486, the add 1: a call in a loop keeps the add
//! (select_of_constants +10% clocks). Needs `LLRM_LLC`.

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
    let body = |name: &str| {
        let start = format!("{name} proc near");
        let end = format!("{name} endp");
        text.split(start.as_str()).nth(1).and_then(|rest| rest.split(end.as_str()).next()).expect("a body").to_owned()
    };
    for straight in ["a", "b"] {
        let code = body(straight);
        assert!(!code.contains("add esp, 4") && !code.contains("add esp, 8"), "{straight} cleaned up by add:\n{code}");
        assert!(code.contains("pop "), "{straight} has no pop cleanup:\n{code}");
    }
    let looped = body("loop");
    // x86_32-linux keeps esp 16-aligned at calls: the loop pads (`sub esp, 12`,
    // `push`, `add esp, 16`), so the cleanup is not the 4-byte `add esp, 4`.
    assert!(looped.contains("add esp, ") && !looped.contains("pop ecx"), "a call in a loop popped:\n{looped}");
}
