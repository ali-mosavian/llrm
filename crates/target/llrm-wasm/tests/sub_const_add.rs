use std::path::Path;
use std::process::Command;

/// `70 - ((i - 2) << 1)` in an unrolled store loop kept the add (`i32.const
/// 70; local.get; i32.const -2; i32.add; i32.sub`) where the pin folds
/// `C2-(A+C1)` into `72 - A`: dot -O2 +18 bytes.
#[test]
fn a_constant_minus_a_sum_with_a_constant_is_one_subtraction() {
    let ir = Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/ir/sub_const_add.ll");
    let out = Command::new(env!("CARGO_BIN_EXE_llrm-wasm-llc"))
        .env("LLRM_WASM_BACKEND_ONLY", "1")
        .args(["-S", "-O2", "-o", "-"])
        .arg(ir)
        .output()
        .expect("llrm-wasm-llc");
    assert!(out.status.success());
    let text = String::from_utf8_lossy(&out.stdout);
    assert!(text.contains("i32.const\t72") || text.contains("i32.const 72"), "{text}");
    assert!(!text.contains("i32.const\t-2") && !text.contains("i32.const -2"), "{text}");
}
