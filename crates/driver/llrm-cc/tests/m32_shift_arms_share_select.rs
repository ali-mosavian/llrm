//! u64_shift_select_arms at m32 -O2 -march=i486: the shift arms of four
//! diamonds were laid out behind the function's end with long jumps (416 B of
//! code, 383 B with the early merge): `BranchFolder` runs ahead of placement,
//! where the arms' select `mov`s, behind edge blocks, are merged first.

use std::path::Path;
use std::process::Command;

#[test]
fn test_m32_o2_the_shift_arms_are_laid_out_after_the_pins_merge() {
    let dir = Path::new(env!("CARGO_TARGET_TMPDIR")).join(format!("shift-arms-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).unwrap();
    let source = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../../tests/run/c/u64_shift_select_arms.c");
    let object = dir.join("c.o");
    let run = Command::new(env!("CARGO_BIN_EXE_llrm-cc-bin"))
        .env("LLRM_CC_TARGET", "x86_32-linux")
        .args(["-m32", "-march=i486", "-O2", "-w", "-c", "-o"])
        .arg(&object)
        .arg(source)
        .output()
        .unwrap();
    assert!(run.status.success(), "{}", String::from_utf8_lossy(&run.stderr));
    let sizes = Command::new("size").arg(&object).output().unwrap();
    let text = String::from_utf8_lossy(&sizes.stdout);
    let code: u32 = text.lines().nth(1).and_then(|row| row.split_whitespace().next()).unwrap().parse().unwrap();
    assert!(code <= 390, "{code} B of code, 383 expected, 416 before the early tail merge:\n{text}");
}
