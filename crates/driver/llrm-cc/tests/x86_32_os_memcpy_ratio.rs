//! gcc moves a constant block by pieces at -Os only while their count is under
//! MOVE_RATIO (3), else `rep movs`. The pin's four pairs copied QuickrBasic's
//! 13-byte `dta.name` as eight moves (dialog.c `find`, 38 B against 7). A
//! `Described` machine's own `max_stores_per_memcpy_opt_size` never reached the
//! selector (`Machine::tuning` was the pin's), so the limit was a no-op.
//! Needs `LLRM_LLC`.

use std::path::Path;
use std::process::Command;

fn listing() -> String {
    let llc = std::env::var_os("LLRM_LLC").expect("LLRM_LLC is not set: this test needs it and does not skip");
    let dir = Path::new(env!("CARGO_TARGET_TMPDIR")).join(format!("x86-os-memcpy-ratio-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).unwrap();
    let object = dir.join("a.o");
    let run = Command::new(env!("CARGO_BIN_EXE_llrm-cc-bin"))
        .env("LLRM_CC_TARGET", "x86_32-linux")
        .env("LLRM_LLC", llc)
        .args(["-march=i486", "-Os", "-w", "-ffreestanding", "-c", "-o"])
        .arg(&object)
        .arg(Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/c/m32_os_memcpy_ratio.c"))
        .output()
        .unwrap();
    assert!(run.status.success(), "{}", String::from_utf8_lossy(&run.stderr));
    let dump = Command::new("objdump").args(["-d", "--no-show-raw-insn", "-M", "intel"]).arg(&object).output().unwrap();
    String::from_utf8_lossy(&dump.stdout).into_owned()
}

#[test]
fn a_constant_copy_of_more_than_two_pieces_is_rep_movs_at_os() {
    let text = listing();
    assert!(text.contains("rep movs"), "a 13-byte copy at -Os is inline pieces\n{text}");
}
