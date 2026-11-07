use std::path::Path;
use std::process::Command;

/// The hand selector read an fp128 zero as a double's and stored `xzr`: 8 of
/// the 16 bytes (`store fp128 zeroinitializer`). The DAG selector lowers it and
/// the store is a whole `q` register (`str q16, [x0]`).
#[test]
fn an_fp128_zero_is_stored_whole() {
    let ir = Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/ir/fp128_store_zero.ll");
    let dir = tempfile::tempdir().unwrap();
    let object = dir.path().join("f.o");
    let out = Command::new(env!("CARGO_BIN_EXE_llrm-arm64-llc"))
        .args(["-O1", "-o"])
        .arg(&object)
        .arg(ir)
        .output()
        .expect("llrm-arm64-llc");
    let stderr = String::from_utf8_lossy(&out.stderr).into_owned();
    assert!(out.status.success(), "{stderr}");
    let bytes = std::fs::read(&object).unwrap();
    let store_q = 0x3d80_0010u32.to_le_bytes();
    assert!(bytes.windows(4).any(|word| word == store_q), "no 16-byte store in the object");
}
