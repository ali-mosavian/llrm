use std::path::Path;
use std::process::Command;

/// The pin cannot split an edge out of a `BR_TABLE` block (`analyzeBranch`
/// fails on it), so a switch's first case that feeds a PHI keeps its copy in
/// the switch block. We split it: the case became a block of
/// `local.get 0; local.set 0; br 3`, a no-op the pin does not have
/// (div_constants -O1, +7 B per function).
#[test]
fn a_switch_case_into_a_phi_gets_no_edge_block() {
    let ir = Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/ir/switch_phi_edge.ll");
    let out = Command::new(env!("CARGO_BIN_EXE_llrm-wasm-llc"))
        .args(["-S", "-O1", "-o", "-"])
        .arg(ir)
        .env("LLRM_WASM_BACKEND_ONLY", "1")
        .output()
        .expect("llrm-wasm-llc");
    assert!(out.status.success());
    let asm = String::from_utf8_lossy(&out.stdout);
    assert!(asm.contains("br_table"), "premise: the switch is a br_table\n{asm}");
    let lines: Vec<&str> = asm.lines().map(str::trim).collect();
    for pair in lines.windows(2) {
        let same = pair[0].strip_prefix("local.get\t").zip(pair[1].strip_prefix("local.set\t"));
        assert!(same.is_none_or(|(get, set)| get != set), "a copy of a local onto itself\n{asm}");
    }
}
