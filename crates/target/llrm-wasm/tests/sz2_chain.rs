use std::path::Path;
use std::process::Command;

/// `call_keeps` -O1 kept `churn` in `hold` and `hold` in `main` (263 B against the pin's 50): the size
/// bound refused each inline and a trial of either alone does not shrink its caller. The pin inlines
/// callee-first and folds the constants, three `report` calls; so must the trial that brings the
/// chain's second level into the first.
#[test]
fn a_chain_of_refused_inlines_that_folds_is_taken_at_o1() {
    let ir = Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/ir/sz2_chain.ll");
    let out = Command::new(env!("CARGO_BIN_EXE_llrm-wasm-llc"))
        .args(["-S", "-O1", "-o", "-"])
        .arg(ir)
        .output()
        .expect("llrm-wasm-llc");
    assert!(out.status.success());
    let asm = String::from_utf8_lossy(&out.stdout).into_owned();
    assert!(!asm.contains("call\thold") && !asm.contains("call\tchurn"), "{asm}");
}
