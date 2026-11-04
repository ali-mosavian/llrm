use std::path::Path;
use std::process::Command;

/// MIR parses `fp128` and this backend has no lowering for it yet: a function
/// that uses one is refused by name, not panicked on and not selected as a
/// double (the float arms read any other kind as one).
#[test]
fn an_fp128_function_is_refused_by_name() {
    let ir = Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/ir/fp128_add.ll");
    let out = Command::new(env!("CARGO_BIN_EXE_llrm-wasm-llc"))
        .args(["-S", "-O1", "-o", "-"])
        .arg(ir)
        .output()
        .expect("llrm-wasm-llc");
    let stderr = String::from_utf8_lossy(&out.stderr).into_owned();
    assert!(!out.status.success(), "{}", String::from_utf8_lossy(&out.stdout));
    assert!(stderr.contains("fp128"), "{stderr}");
    assert!(!stderr.contains("panicked"), "{stderr}");
}
