//! x86-64 -Os: an aggregate zeroed and then set field by field is one `rep
//! stos`. SROA split the memset where a field is set, so QuickrBasic's
//! `forloop` cleared 104 bytes with two (73 and 23 bytes; gcc: one). Needs
//! `LLRM_LLC`.

use std::path::Path;
use std::process::Command;

#[test]
fn a_zeroed_aggregate_set_field_by_field_is_cleared_once() {
    let llc = std::env::var_os("LLRM_LLC").expect("LLRM_LLC is not set: this test needs it and does not skip");
    let dir = Path::new(env!("CARGO_TARGET_TMPDIR")).join(format!("memset-gap-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).unwrap();
    let asm = dir.join("a.s");
    let run = Command::new(env!("CARGO_BIN_EXE_llrm-cc-bin"))
        .env("LLRM_CC_TARGET", "x86_64-linux")
        .env("LLRM_LLC", llc)
        .args(["-Os", "-w", "-S", "-o"])
        .arg(&asm)
        .arg(Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/c/memset_gap.c"))
        .output()
        .unwrap();
    assert!(run.status.success(), "{}", String::from_utf8_lossy(&run.stderr));
    let text = std::fs::read_to_string(&asm).unwrap();
    let clears = text.matches("rep stos").count();
    assert_eq!(clears, 1, "{clears} string clears:\n{text}");
}
