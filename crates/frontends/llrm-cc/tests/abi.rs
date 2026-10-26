//! Each target's `tests/cc/abi/<name>.c` compiles: its `_Static_assert`s state the sizes and layouts clang is told.
//! m16 needs the i8086 clang (`LLRM_CLANG_M16`); without it the check says it skipped.

use std::path::Path;
use std::process::Command;

fn compile(target: &str, file: &str) -> Result<(), String> {
    let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../..");
    let out = Command::new(env!("CARGO_BIN_EXE_llrm-cc"))
        .env("LLRM_CC_TARGET", target)
        .env("LLRM_LLC", root.join("target/release/llrm-llc"))
        .args(["-c", "-O1", "-o", "/dev/null"])
        .arg(root.join("tests/cc/abi").join(file))
        .output()
        .unwrap();
    if out.status.success() { Ok(()) } else { Err(String::from_utf8_lossy(&out.stderr).into_owned()) }
}

#[test]
fn m16_sizes_and_layouts_are_the_ones_llrm_expects() {
    if std::env::var_os("LLRM_CLANG_M16").is_none() {
        eprintln!("skipped: LLRM_CLANG_M16 names no i8086 clang");
        return;
    }
    compile("x86_16-dos", "x86_16.c").unwrap();
}
