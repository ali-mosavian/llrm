use std::path::Path;
use std::process::Command;

/// `MachineLICM` took the blocks in block-number order, not in the order of the
/// dominator tree's children, so the `CONST 8` it kept in the preheader was the
/// compare's own, a last use: `x <u 8` left the outer loop, where the pin's
/// constant is shared and the compare stays (spill_store_order -O2: 2
/// instructions and 4 bytes over the pin).
#[test]
fn a_compare_whose_constant_the_loop_shares_is_not_hoisted() {
    let ir = Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/ir/licm_visit_order.ll");
    let out = Command::new(env!("CARGO_BIN_EXE_llrm-wasm-llc"))
        .env("LLRM_WASM_BACKEND_ONLY", "1")
        .args(["-O2", "-S", "-o", "-"])
        .arg(ir)
        .output()
        .expect("llrm-wasm-llc");
    assert!(out.status.success(), "{}", String::from_utf8_lossy(&out.stderr));
    let text = String::from_utf8_lossy(&out.stdout);
    let lines: Vec<&str> = text.lines().map(str::trim).collect();
    let first_loop = lines.iter().position(|l| l.starts_with("loop")).expect("a loop");
    let compare = lines.iter().position(|l| l.starts_with("i32.lt_u")).expect("the compare");
    assert!(compare > first_loop, "i32.lt_u at line {compare} is before the loop at {first_loop}:\n{text}");
}
