use std::path::Path;
use std::process::Command;

/// MIR parses `fp128` and this backend has no lowering for it yet: a function
/// that uses one is refused by name. The hand selector read an fp128 zero as a
/// double's and stored `xzr`: 8 of the 16 bytes (`store fp128
/// zeroinitializer`).
#[test]
fn an_fp128_function_is_refused_by_name() {
    let ir = Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/ir/fp128_store_zero.ll");
    let out = Command::new(env!("CARGO_BIN_EXE_llrm-arm64-llc"))
        .args(["-S", "-O1", "-o", "-"])
        .arg(ir)
        .output()
        .expect("llrm-arm64-llc");
    let stderr = String::from_utf8_lossy(&out.stderr).into_owned();
    assert!(!out.status.success(), "{}", String::from_utf8_lossy(&out.stdout));
    assert!(stderr.contains("fp128"), "{stderr}");
    assert!(!stderr.contains("panicked"), "{stderr}");
}
