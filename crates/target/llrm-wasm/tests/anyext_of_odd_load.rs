use std::path::Path;
use std::process::Command;

/// `shl (zext (load i24)), 40` demands only the low 24 bits, so
/// SimplifyDemandedBits makes the `zext` an `anyext`. The replaced `zext`
/// stayed in the DAG as a second reader of the load, so the load was not folded
/// into an extending load: two i32 loads, an `or` and an `i64.extend_i32_u`
/// where the pin loads both pieces as i64 (wasm bitfield_long_long -O2/-O3/-Os,
/// +1 B).
#[test]
fn an_odd_sized_load_extended_to_i64_is_loaded_as_i64_pieces() {
    let ir = Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/ir/anyext_of_odd_load.ll");
    let out = Command::new(env!("CARGO_BIN_EXE_llrm-wasm-llc"))
        .args(["-O2", "-S", "-o", "-"])
        .arg(ir)
        .output()
        .expect("llrm-wasm-llc");
    assert!(out.status.success());
    let text = String::from_utf8_lossy(&out.stdout);
    assert!(text.contains("i64.load16_u"), "{text}");
    assert!(!text.contains("i64.extend_i32_u"), "{text}");
}
