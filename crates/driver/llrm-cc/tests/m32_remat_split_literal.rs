//! A literal that the split editor made again in two blocks (`MOV32ri` twice
//! into one piece) was not rematerialized when the piece was spilled: sfm16
//! `__mulsf3` m32 `-Os` stored `8388608` to a stack slot and read it back as a
//! memory operand, 20 B more than the pin, which remakes it before each use.
//! The fixture is the function as the backend receives it (the last mid-end
//! stage of `sfm16_float_add`).

#[path = "common/target_dir.rs"]
mod target_dir;

use std::path::Path;
use std::process::Command;

#[test]
fn test_a_literal_defined_twice_in_a_split_piece_is_remade_not_spilled() {
    let dir = Path::new(env!("CARGO_TARGET_TMPDIR")).join(format!("m32remat-{}", std::process::id()));
    let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../..");
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).unwrap();
    let llc = std::env::var_os("LLRM_LLC")
        .map(std::path::PathBuf::from)
        .unwrap_or_else(|| target_dir::of(&root).join("release/llrm-llc"));
    let fixture = Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/ll/m32_remat_split_literal.ll");
    let done = Command::new(llc)
        .args(["-m32", "-Os", "-march=i486", "--isel=dag", "-mabi=sysv", "-S", "-o"])
        .arg(dir.join("d.s"))
        .arg(&fixture)
        .output()
        .unwrap();
    assert!(done.status.success(), "{}", String::from_utf8_lossy(&done.stderr));
    let asm = std::fs::read_to_string(dir.join("d.s")).unwrap();

    // Premise: the literal is still made at least twice around the two
    // `__clzsi2` calls.
    assert!(asm.matches(", 8388608").count() >= 2, "the fixture no longer makes the literal twice:\n{asm}");
    assert_eq!(asm.lines().filter(|l| l.trim_start().starts_with("call") && l.contains("clzsi2")).count(), 2, "{asm}");

    let stored: Vec<&str> =
        asm.lines().filter(|l| l.contains("ptr [esp") && l.trim_end().ends_with(", 8388608")).collect();
    assert!(stored.is_empty(), "the literal was spilled to the stack instead of remade: {stored:?}");
}
