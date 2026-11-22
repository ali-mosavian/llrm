//! A byte `switch` whose cases each call and return an error early, at -Os on
//! m32. The pin's `MachineBlockPlacement` put the error exits out of line, so
//! 30 of the 35 conditional jumps were `jcc rel32` (6 B) and `.text` was 668 B;
//! `-apply-ext-tsp-for-size` keeps the jumps near (586 B). QuickrBasic's
//! ctrl_stmt was +39 long jumps over gcc. Needs `LLRM_LLC`.

use std::path::Path;
use std::process::Command;

#[test]
fn a_dispatch_at_os_keeps_its_conditional_jumps_short() {
    let llc = std::env::var_os("LLRM_LLC").expect("LLRM_LLC is not set: this test needs it and does not skip");
    let dir = Path::new(env!("CARGO_TARGET_TMPDIR")).join(format!("x86-dispatch-layout-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).unwrap();
    let object = dir.join("a.o");
    let run = Command::new(env!("CARGO_BIN_EXE_llrm-cc-bin"))
        .env("LLRM_CC_TARGET", "x86_32-linux")
        .env("LLRM_LLC", llc)
        .args(["-march=i486", "-Os", "-w", "-ffreestanding", "-c", "-o"])
        .arg(&object)
        .arg(Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/c/m32_os_dispatch_layout.c"))
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
    assert!(bytes <= 620, ".text is {bytes} B (the pin's placement: 668, ext-tsp for size: 586)");
}
