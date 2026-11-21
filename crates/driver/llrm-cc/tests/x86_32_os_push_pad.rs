//! A call whose arguments are not a whole alignment step, at -Os on m32. The
//! four-byte pad was `sub esp,4` (3 B) where gcc's `i386.md` peephole2 pushes
//! a dead register (1 B): `.text` was 121 B, 117 B with the push.
//! QuickrBasic m32 Os has 330 such pads. Needs `LLRM_LLC`.

use std::path::Path;
use std::process::Command;

#[test]
fn a_four_byte_call_pad_at_os_is_a_push_not_a_sub() {
    let llc = std::env::var_os("LLRM_LLC").expect("LLRM_LLC is not set: this test needs it and does not skip");
    let dir = Path::new(env!("CARGO_TARGET_TMPDIR")).join(format!("x86-push-pad-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).unwrap();
    let object = dir.join("a.o");
    let run = Command::new(env!("CARGO_BIN_EXE_llrm-cc-bin"))
        .env("LLRM_CC_TARGET", "x86_32-linux")
        .env("LLRM_LLC", llc)
        .args(["-march=i486", "-Os", "-fomit-frame-pointer", "-w", "-ffreestanding", "-c", "-o"])
        .arg(&object)
        .arg(Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/c/m32_os_push_pad.c"))
        .output()
        .unwrap();
    assert!(run.status.success(), "{}", String::from_utf8_lossy(&run.stderr));
    let sizes = Command::new("size").arg("-A").arg(&object).output().unwrap();
    let text = String::from_utf8_lossy(&sizes.stdout).into_owned();
    let bytes: u32 = text
        .lines()
        .find(|line| line.starts_with(".text"))
        .and_then(|line| line.split_whitespace().nth(1))
        .and_then(|bytes| bytes.parse().ok())
        .unwrap_or_else(|| panic!("no .text in:\n{text}"));
    assert!(bytes <= 118, ".text is {bytes} B (the pad as `sub esp,4`: 121, as a push: 117)");
}
