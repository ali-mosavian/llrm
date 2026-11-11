use std::path::Path;
use std::process::Command;

/// `splitMergedValStore` runs on wasm and does nothing: its hook,
/// `isMultiStoresCheaperThanBitsMerge`, is the base class's `false` (x86
/// alone says yes for a float and an integer), so the merged store stays one
/// `i64.store`. A premise test: no fail-first, the step has no effect here;
/// it fails if the hook ever says yes.
#[test]
fn a_float_and_an_integer_merged_into_one_store_stay_one_store() {
    let ir = Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/ir/merged_float_and_int_store.ll");
    let out = Command::new(env!("CARGO_BIN_EXE_llrm-wasm-llc"))
        .args(["-S", "-O2", "-o", "-"])
        .arg(ir)
        .output()
        .expect("llrm-wasm-llc");
    assert!(out.status.success());
    let asm = String::from_utf8_lossy(&out.stdout);
    assert_eq!(asm.matches(".store").count(), 1, "{asm}");
    assert!(asm.contains("i64.store"), "{asm}");
}
