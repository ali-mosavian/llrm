use std::path::Path;
use std::process::Command;

/// A branch on the negation of an `i1` phi was `local.get; i32.const 1;
/// i32.xor; br_if` (the pin's `rebuildSetCC` makes the `xor` a compare, which
/// is `i32.eqz`): jpeg -O1 +6 bytes in three places.
#[test]
fn a_branch_on_a_negated_boolean_is_eqz() {
    let ir = Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/ir/brcond_not.ll");
    let out = Command::new(env!("CARGO_BIN_EXE_llrm-wasm-llc"))
        .args(["-S", "-O1", "-o", "-"])
        .arg(ir)
        .output()
        .expect("llrm-wasm-llc");
    assert!(out.status.success());
    let text = String::from_utf8_lossy(&out.stdout);
    assert!(!text.contains("i32.xor"), "{text}");
}

/// `select %c, %c, false` branched on made `x < 32; i32.const 1; i32.ne` where
/// `x > 31` was one compare (sieve -O1 +3 bytes, +344 instructions): the
/// `xor` became a compare of a compare before the select settled.
#[test]
fn a_branch_on_a_compare_of_a_compare_is_the_compare() {
    let ir = Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/ir/brcond_same.ll");
    let out = Command::new(env!("CARGO_BIN_EXE_llrm-wasm-llc"))
        .env("LLRM_WASM_BACKEND_ONLY", "1")
        .args(["-S", "-O1", "-o", "-"])
        .arg(ir)
        .output()
        .expect("llrm-wasm-llc");
    assert!(out.status.success());
    let text = String::from_utf8_lossy(&out.stdout);
    assert!(text.contains("i32.gt_u"), "{text}");
}
