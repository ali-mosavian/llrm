use std::path::Path;
use std::process::Command;

fn i64_stores(ir: &str) -> usize {
    let ir = Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/ir").join(ir);
    let out = Command::new(env!("CARGO_BIN_EXE_llrm-wasm-llc"))
        .args(["-S", "-O2", "-o", "-"])
        .arg(ir)
        .output()
        .expect("llrm-wasm-llc");
    assert!(out.status.success());
    String::from_utf8_lossy(&out.stdout).matches("i64.store").count()
}

/// Dead token factors stayed users of their operands, the chains of the
/// interleaved i16 stores nested past the alias walk's depth 18 and the far
/// constant stores never reached the merge: matmul -O2 made 2 `i64.store`
/// (the pin 6).
#[test]
fn matmul_constant_stores_merge_into_more_than_two_i64_stores() {
    let wide = i64_stores("matmul_store_merge.ll");
    assert!(wide >= 3, "{wide} i64.store");
}

/// The merge stopped once types were legal, so the promoted i16 constant
/// stores (truncating i32 stores) of the last run stayed apart: 2 `i64.store`
/// where the pin makes 3.
#[test]
fn promoted_constant_stores_merge_after_legalization() {
    let wide = i64_stores("storemerge_min16.ll");
    assert!(wide >= 3, "{wide} i64.store");
}

fn i64_loads_backend_only(ir: &str) -> usize {
    let ir = Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/ir").join(ir);
    let out = Command::new(env!("CARGO_BIN_EXE_llrm-wasm-llc"))
        .args(["-S", "-O2", "-o", "-"])
        .arg(ir)
        .env("LLRM_WASM_BACKEND_ONLY", "1")
        .output()
        .expect("llrm-wasm-llc");
    assert!(out.status.success());
    String::from_utf8_lossy(&out.stdout).matches("i64.load").count()
}

/// A load replaced by the type legalizer stayed in the DAG as a user of its
/// chain, so the chain looked shared and the last six `i16` copies of a
/// 44-pair run never merged: 8 `i64.load` (the pin 9), 72 bytes more.
#[test]
fn a_replaced_load_does_not_stop_the_tail_of_a_copy_run_from_merging() {
    let wide = i64_loads_backend_only("copy_run_merge.ll");
    assert!(wide >= 9, "{wide} i64.load");
}
