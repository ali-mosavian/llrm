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

/// `(x*a + c) * K1 + ((x*a + c) * a + c) * 3`: `visitADDLike` folds
/// `add(mul(add(A, CA), CM), CB)` to `add(mul(A, CM), CM*CA + CB)` even where
/// the inner add has other users, so both products read `x`. The wasm DAG kept
/// the shared `x*a + c` and five multiplies, the pin two (`mul_constants`).
#[test]
fn a_chain_of_sums_times_constants_reads_the_variable_not_the_shared_sum() {
    let ir = Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/ir/mul_add_chain.ll");
    let out = Command::new(env!("CARGO_BIN_EXE_llrm-wasm-llc"))
        .args(["-S", "-O2", "-o", "-"])
        .arg(ir)
        .env("LLRM_WASM_BACKEND_ONLY", "1")
        .output()
        .expect("llrm-wasm-llc");
    assert!(out.status.success());
    let asm = String::from_utf8_lossy(&out.stdout);
    assert_eq!(asm.matches("i32.mul").count(), 2, "{asm}");
}
