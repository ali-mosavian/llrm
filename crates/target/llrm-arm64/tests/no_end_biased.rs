//! LSR's end-biased formulae (DEVIATE LG3) are not the pin's. On arm64 their
//! `@sym+24` bases cost an `adrp`+`add` each: nbody_single -O2 made 8 `adrp`
//! where the pin's solution needs 4 (+12 B, 2 more executed instructions).

use std::process::Command;

#[test]
fn nbody_single_keeps_the_plain_symbol_bases() {
    let dir = tempfile::tempdir().unwrap();
    let output = dir.path().join("t.s");
    let said = Command::new(env!("CARGO_BIN_EXE_llrm-arm64-llc"))
        .env_remove("LLRM_ARM64_VECTORS")
        .args(["-S", "-O2", "-o"])
        .arg(&output)
        .arg(concat!(env!("CARGO_MANIFEST_DIR"), "/tests/data/nbody_single_o2.ll"))
        .output()
        .unwrap();
    assert!(said.status.success(), "{}", String::from_utf8_lossy(&said.stderr));
    let text = std::fs::read_to_string(&output).unwrap();
    let pages = text.lines().filter(|line| line.contains("adrp")).count();
    assert_eq!(pages, 4, "{pages} adrp\n{text}");
}
