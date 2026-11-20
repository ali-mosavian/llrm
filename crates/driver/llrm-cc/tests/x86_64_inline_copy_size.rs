//! x86-64 -Os: a callee that is a store, a 48-byte copy and a call stays a
//! call from callers that pass it a constant. The size guard counted the copy
//! as one instruction, so QuickrBasic's `them_dark` carried three vector
//! loads and stores (37 bytes) for a 10-byte call. Needs `LLRM_LLC`.

use std::path::Path;
use std::process::Command;

fn assembly() -> String {
    let llc = std::env::var_os("LLRM_LLC").expect("LLRM_LLC is not set: this test needs it and does not skip");
    let dir = Path::new(env!("CARGO_TARGET_TMPDIR")).join(format!("inline-copy-size-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).unwrap();
    let asm = dir.join("a.s");
    let run = Command::new(env!("CARGO_BIN_EXE_llrm-cc-bin"))
        .env("LLRM_CC_TARGET", "x86_64-linux")
        .env("LLRM_LLC", llc)
        .args(["-Os", "-w", "-S", "-o"])
        .arg(&asm)
        .arg(Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/c/inline_copy_size.c"))
        .output()
        .unwrap();
    assert!(run.status.success(), "{}", String::from_utf8_lossy(&run.stderr));
    std::fs::read_to_string(&asm).unwrap()
}

fn body<'a>(
    text: &'a str,
    name: &str,
) -> &'a str {
    let start = format!("{name} proc near");
    let end = format!("{name} endp");
    text.split(start.as_str()).nth(1).and_then(|rest| rest.split(end.as_str()).next()).expect("a body")
}

#[test]
fn a_caller_of_a_store_and_a_48_byte_copy_keeps_the_call() {
    let text = assembly();
    for caller in ["dark", "light"] {
        let code = body(&text, caller);
        assert!(code.contains("set_theme") && !code.contains("xmmword"), "{caller} inlined the copy:\n{code}");
    }
}
