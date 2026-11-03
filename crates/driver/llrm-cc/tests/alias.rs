//! `int other(int) __attribute__((alias("real")))` for x86: the IR's `alias`
//! line stopped the parser, then the x86 writer refused it. It is a second
//! public name at the aliasee's offset: both names are in the OMF object at
//! m16 (the compiler-rt compare routines alias their entry points).

#[path = "common/target_dir.rs"]
mod target_dir;

use std::path::Path;
use std::process::Command;

#[test]
fn a_c_alias_is_a_public_name_in_the_m16_object() {
    let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../..");
    let object = Path::new(env!("CARGO_TARGET_TMPDIR")).join(format!("alias-{}.o", std::process::id()));
    let out = Command::new(env!("CARGO_BIN_EXE_llrm-cc"))
        .env("LLRM_CC_TARGET", "x86_16-dos")
        .env("LLRM_LLC", target_dir::of(&root).join("release/llrm-llc"))
        .args(["-O2", "-w", "-c", "-march=i386", "-o"])
        .arg(&object)
        .arg(root.join("tests/cc/alias/alias.c"))
        .output()
        .unwrap();
    assert!(out.status.success(), "{}", String::from_utf8_lossy(&out.stderr));
    let bytes = std::fs::read(&object).unwrap();
    let has = |name: &[u8]| bytes.windows(name.len()).any(|window| window == name);
    assert!(has(b"_real") && has(b"_other"), "a name is missing from the object");
}
