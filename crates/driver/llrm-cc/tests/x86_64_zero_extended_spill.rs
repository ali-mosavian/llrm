//! x86-64: a value every definition of which is a dword write (`add eax, 1`) is
//! zero above bit 31 wherever it lives, but spilled in the register's 8-byte
//! cell. jpeg_single -Os spilled 19 of its 25 slots at 8 bytes where gcc packs
//! 4-byte ints: 41 `[rsp+disp32]` operands against gcc's 22, +48 B. Needs
//! `LLRM_LLC`.

use std::path::Path;
use std::process::Command;

#[test]
fn a_dword_defined_value_spills_in_a_dword_cell_at_x86_64() {
    let llc = std::env::var_os("LLRM_LLC").expect("LLRM_LLC is not set: this test needs it and does not skip");
    let dir = Path::new(env!("CARGO_TARGET_TMPDIR")).join(format!("zext-spill-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).unwrap();
    let asm = dir.join("s.s");
    let run = Command::new(env!("CARGO_BIN_EXE_llrm-cc-bin"))
        .env("LLRM_CC_TARGET", "x86_64-linux")
        .env("LLRM_LLC", llc)
        .args(["-Os", "-w", "-S", "-o"])
        .arg(&asm)
        .arg(Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/c/zext_spill.c"))
        .output()
        .unwrap();
    assert!(run.status.success(), "{}", String::from_utf8_lossy(&run.stderr));
    let text = std::fs::read_to_string(&asm).unwrap();
    let wide: Vec<&str> = text.lines().filter(|line| line.contains("qword ptr [rsp")).collect();
    assert!(wide.is_empty(), "values set by a dword write spilled whole:\n{}", wide.join("\n"));
    assert!(text.lines().any(|line| line.contains("dword ptr [rsp")), "no spill at all:\n{text}");
}
