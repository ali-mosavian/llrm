//! A pre-indexed store updates its base register in place. `MachineCSE`'s
//! phi heuristic read that second definition as a phi's second input and
//! refused to share the symbol's `add :lo12:` with a later block: lru -O3 made
//! it nine times (115 instructions; the pin shares it once, 99).

use std::process::Command;

fn compile(
    fixture: &str,
    level: &str,
) -> String {
    let dir = tempfile::tempdir().unwrap();
    let output = dir.path().join("t.s");
    let said = Command::new(env!("CARGO_BIN_EXE_llrm-arm64-llc"))
        .env("LLRM_ARM64_VECTORS", "0")
        .args(["-S", level, "-o"])
        .arg(&output)
        .arg(format!("{}/tests/data/{fixture}.ll", env!("CARGO_MANIFEST_DIR")))
        .output()
        .unwrap();
    assert!(said.status.success(), "{}", String::from_utf8_lossy(&said.stderr));
    std::fs::read_to_string(&output).unwrap()
}

#[test]
fn lru_o3_shares_the_symbol_base_add_past_a_pre_indexed_store() {
    let text = compile("bench_lru", "-O3");
    let adds = text
        .lines()
        .filter(|line| line.trim_start().starts_with("add ") && line.contains("_MergedGlobals@PAGEOFF"))
        .count();
    assert!(adds <= 2, "{adds} symbol base adds:\n{text}");
}
