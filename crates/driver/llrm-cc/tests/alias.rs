//! `int other(int) __attribute__((alias("real")))` compiles for x86-64: the
//! IR's `alias` line stopped the parser at "expected `global` or `constant`,
//! found `alias`", so any C file with an alias failed on every target.

#[path = "common/target_dir.rs"]
mod target_dir;

use std::path::Path;
use std::process::Command;

#[test]
fn a_c_alias_compiles_for_x86_64() {
    let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../..");
    let out = Command::new(env!("CARGO_BIN_EXE_llrm-cc"))
        .env("LLRM_CC_TARGET", "x86_64-linux")
        .env("LLRM_LLC", target_dir::of(&root).join("release/llrm-llc"))
        .args(["-O2", "-S", "-o", "/dev/stdout"])
        .arg(root.join("tests/cc/alias/alias.c"))
        .output()
        .unwrap();
    assert!(out.status.success(), "{}", String::from_utf8_lossy(&out.stderr));
    assert!(String::from_utf8_lossy(&out.stdout).contains("real proc"));
}
