use std::path::Path;
use std::process::Command;

/// SROA cuts a three-byte slice into an `i24` (SROA.cpp `isIntegerWideningViableForSlice`), which the
/// wasm DAG legalizes. We kept the slice whole: `i = f(i)` stayed an `i64` load, mask and or, `report(i.a)`
/// was not the constant 12, and bitfield_long_long was 12 B over the pin at every level.
#[test]
fn a_struct_copy_is_cut_at_the_odd_sized_slice_and_the_stored_byte_is_forwarded() {
    let ir = Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/ir/sroa_odd_slice.ll");
    let out = Command::new(env!("CARGO_BIN_EXE_llrm-wasm-llc"))
        .args(["-S", "-O2", "-o", "-"])
        .arg(ir)
        .output()
        .expect("llrm-wasm-llc");
    assert!(out.status.success());
    let text = String::from_utf8_lossy(&out.stdout);
    let masks = text.lines().filter(|line| line.trim().ends_with(".and")).count();
    assert_eq!(masks, 0, "{masks} masks:\n{text}");
}
