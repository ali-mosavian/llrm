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

/// Four stages of `x = x * a + c; total += x * k`: the combiner took a node the
/// fold had just made after its users, not before them
/// (`AddToWorklistWithUsers` puts the new node first), so the stage-3 sum kept
/// its two users and six multiplies stayed where the pin has five
/// (`mul_constants`, -O2 +497 B).
#[test]
fn a_node_a_fold_made_is_combined_before_its_users() {
    let ir = Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/ir/mul_add_four_stages.ll");
    let out = Command::new(env!("CARGO_BIN_EXE_llrm-wasm-llc"))
        .args(["-S", "-O2", "-o", "-"])
        .arg(ir)
        .env("LLRM_WASM_BACKEND_ONLY", "1")
        .output()
        .expect("llrm-wasm-llc");
    assert!(out.status.success());
    let asm = String::from_utf8_lossy(&out.stdout);
    assert_eq!(asm.matches("i32.mul").count(), 5, "{asm}");
}
