//! The -Os size bound kept `blend` a call in the inner loop of a fill (a site 961 times the
//! caller's entry), where the pin's `optsize` threshold takes it: QuickrBasic `pixmem.c`, `q45d25`
//! 2,335,019 instructions against clang's 1,443,306. Needs `LLRM_LLC`.

use std::path::Path;
use std::process::Command;

fn listing() -> String {
    let llc = std::env::var_os("LLRM_LLC").expect("LLRM_LLC is not set: this test needs it and does not skip");
    let dir = Path::new(env!("CARGO_TARGET_TMPDIR")).join(format!("x86-os-hot-site-inline-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).unwrap();
    let object = dir.join("a.o");
    let run = Command::new(env!("CARGO_BIN_EXE_llrm-cc-bin"))
        .env("LLRM_CC_TARGET", "x86_32-linux")
        .env("LLRM_LLC", llc)
        .args(["-march=i486", "-Os", "-w", "-ffreestanding", "-c", "-o"])
        .arg(&object)
        .arg(Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/c/m32_os_hot_site_inline.c"))
        .output()
        .unwrap();
    assert!(run.status.success(), "{}", String::from_utf8_lossy(&run.stderr));
    let dump = Command::new("objdump").args(["-d", "--no-show-raw-insn", "-M", "intel"]).arg(&object).output().unwrap();
    String::from_utf8_lossy(&dump.stdout).into_owned()
}

#[test]
fn a_blend_in_a_fills_inner_loop_is_inlined_at_os() {
    let text = listing();
    let fill = text.split("<fill>:").nth(1).expect("fill");
    let body = fill.split("\n\n").next().unwrap();
    assert!(!body.contains("call"), "fill still calls blend in its loop:\n{body}");
}
