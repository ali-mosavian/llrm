use std::path::Path;
use std::process::Command;

/// Dead token factors stayed users of their operands, the chains of the
/// interleaved i16 stores nested past the alias walk's depth 18 and the far
/// constant stores never reached the merge: matmul -O2 made 2 `i64.store`
/// (the pin 6).
#[test]
fn matmul_constant_stores_merge_into_more_than_two_i64_stores() {
    let ir = Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/ir/matmul_store_merge.ll");
    let out = Command::new(env!("CARGO_BIN_EXE_llrm-wasm-llc"))
        .args(["-S", "-O2", "-o", "-"])
        .arg(ir)
        .output()
        .expect("llrm-wasm-llc");
    assert!(out.status.success());
    let wide = String::from_utf8_lossy(&out.stdout).matches("i64.store").count();
    assert!(wide >= 3, "{wide} i64.store");
}
