use std::path::Path;
use std::process::Command;

/// `(x + 3) * 5 + x * 5`: `visitMUL` folds `(mul (add x, c1), c2)` to `(add
/// (mul x, c2), c1*c2)`, which wasm never did, so `x * 5` was multiplied twice
/// and the unrolled `mul_constants` recurrences kept every multiply (-O2:
/// 4500 B, the pin 3693).
#[test]
fn a_multiply_of_a_sum_by_a_constant_distributes_and_shares_the_product() {
    let ir = Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/ir/mul_add_const.ll");
    let out = Command::new(env!("CARGO_BIN_EXE_llrm-wasm-llc"))
        .args(["-S", "-O2", "-o", "-"])
        .arg(ir)
        .env("LLRM_WASM_BACKEND_ONLY", "1")
        .output()
        .expect("llrm-wasm-llc");
    assert!(out.status.success());
    let asm = String::from_utf8_lossy(&out.stdout);
    assert_eq!(asm.matches("i32.mul").count(), 1, "{asm}");
}
