use std::path::Path;
use std::process::Command;

/// CodeGenPrepare's `optimizeCmp` did not run on wasm, so `icmp ult %b, %a`
/// next to `sub %a, %b` kept its operands: `i32.ge_u` where the pin swaps
/// them to expose the shared subtraction and emits `i32.le_u`
/// (bench/recchop -Os, same size, different instruction).
#[test]
fn a_compare_beside_a_sub_of_the_same_operands_is_swapped_like_the_pin() {
    let ir = Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/ir/compare_swapped_for_sub.ll");
    let out = Command::new(env!("CARGO_BIN_EXE_llrm-wasm-llc"))
        .args(["-S", "-O2", "-o", "-"])
        .arg(ir)
        .output()
        .expect("llrm-wasm-llc");
    assert!(out.status.success());
    let asm = String::from_utf8_lossy(&out.stdout);
    assert!(asm.contains("i32.le_u"), "{asm}");
    assert!(!asm.contains("i32.ge_u"), "{asm}");
}
