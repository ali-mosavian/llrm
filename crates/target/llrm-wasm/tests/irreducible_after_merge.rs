use std::path::Path;
use std::process::Command;

/// A jump-threaded state machine (the object gate's x_switch) was reducible
/// when `FixIrreducibleControlFlow` ran and not after branch folding:
/// `cfg_sort` panicked at -O2 ("active sort region list not finished").
#[test]
fn a_loop_with_several_entries_made_by_branch_folding_still_compiles() {
    let ir = Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/ir/irreducible_after_merge.ll");
    let out = Command::new(env!("CARGO_BIN_EXE_llrm-wasm-llc"))
        .args(["-O2", "-S", "-o", "-"])
        .env("LLRM_WASM_BACKEND_ONLY", "1")
        .arg(ir)
        .output()
        .expect("llrm-wasm-llc");
    assert!(out.status.success(), "{}", String::from_utf8_lossy(&out.stderr));
}
