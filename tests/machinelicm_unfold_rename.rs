//! MachineLICM unfolded `addss xmm, [const]` into a load plus `addss`, then
//! renamed that load into an identical hoisted one; the `addss` kept the old
//! value and liveness panicked ("a unique value", x86-64 jpeg_single -O2).

#[test]
fn an_unfolded_constant_load_renamed_into_a_hoisted_twin_still_has_a_def() {
    let fixture = concat!(env!("CARGO_MANIFEST_DIR"), "/tests/check/mir/licm_unfold_rename.ll");
    let output = std::process::Command::new(env!("CARGO_BIN_EXE_llrm-llc"))
        .args(["-O2", "-S", "-o", "-", fixture])
        .output()
        .unwrap();
    assert!(output.status.success(), "{}", String::from_utf8_lossy(&output.stderr));
}
