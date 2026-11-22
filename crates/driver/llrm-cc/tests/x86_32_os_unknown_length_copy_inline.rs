//! x86-32 -Os: pricing an unknown-length fill or copy as a call in the size
//! trial's units shifted its margin, so `sumlong` was copied into `main` of
//! `tests/run/c/idioms.c` and the object grew from 1074 to 1168 B. Only the
//! inline guard prices that call. Needs `LLRM_LLC`.

use std::path::Path;
use std::process::Command;

#[test]
fn idioms_keeps_sumlong_out_of_line_at_os() {
    let llc = std::env::var_os("LLRM_LLC").expect("LLRM_LLC is not set: this test needs it and does not skip");
    let dir = Path::new(env!("CARGO_TARGET_TMPDIR")).join(format!("os-unknown-length-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).unwrap();
    let asm = dir.join("a.s");
    let run = Command::new(env!("CARGO_BIN_EXE_llrm-cc-bin"))
        .env("LLRM_CC_TARGET", "x86_32-linux")
        .env("LLRM_LLC", llc)
        .args(["-Os", "-w", "-S", "-o"])
        .arg(&asm)
        .arg(Path::new(env!("CARGO_MANIFEST_DIR")).join("../../../tests/run/c/idioms.c"))
        .output()
        .unwrap();
    assert!(run.status.success(), "{}", String::from_utf8_lossy(&run.stderr));
    let text = std::fs::read_to_string(&asm).unwrap();
    assert!(text.contains("sumlong proc"), "sumlong was inlined into main:\n{text}");
}
