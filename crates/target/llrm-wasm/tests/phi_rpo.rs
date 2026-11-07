use std::path::Path;
use std::process::Command;

fn ands() -> usize {
    let ir = Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/ir/phi_rpo.ll");
    let out = Command::new(env!("CARGO_BIN_EXE_llrm-wasm-llc"))
        .args(["-S", "-O1", "-o", "-"])
        .arg(ir)
        .env("LLRM_WASM_BACKEND_ONLY", "1")
        .output()
        .expect("llrm-wasm-llc");
    assert!(out.status.success());
    String::from_utf8_lossy(&out.stdout).matches("i32.and").count()
}

/// Blocks were selected in layout order, so a block laid out before the block
/// of the constant-i1 phi it branches on was built without the phi's known
/// bits and kept `i32.and 1` before `br_if` (bench_jpeg_fixed -O1, +5 B per
/// site). The pin selects in reverse post-order; only the `i1` argument is
/// masked.
#[test]
fn a_branch_on_a_constant_phi_laid_out_before_it_has_no_mask() {
    assert_eq!(ands(), 1, "only the i1 argument is masked");
}
