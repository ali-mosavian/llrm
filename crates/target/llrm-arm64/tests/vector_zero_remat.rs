//! `MOVIv2d_ns` is `isReMaterializable` and `isAsCheapAsAMove`: a copy of a
//! zero vector is a `movi` again.

/// QuickrBasic a64 -O2 (table.c `tabl_moved`): a reduction's zero start
/// vector, shared by two uses, was copied (`mov v17.16b, v16.16b`, 14 of them
/// in the function, 56 B) where the pin makes a second `movi`.
#[test]
fn a_copy_of_a_zero_vector_is_a_second_movi() {
    let dir = tempfile::tempdir().unwrap();
    let (input, output) = (dir.path().join("t.ll"), dir.path().join("t.s"));
    std::fs::write(&input, include_str!("data/vector_zero_remat.ll")).unwrap();
    let said = std::process::Command::new(env!("CARGO_BIN_EXE_llrm-arm64-llc"))
        .args(["-S", "-O2", "-o"])
        .arg(&output)
        .arg(&input)
        .output()
        .unwrap();
    assert!(said.status.success(), "{}", String::from_utf8_lossy(&said.stderr));
    let asm = std::fs::read_to_string(&output).unwrap();
    assert_eq!(asm.matches("movi").count(), 2, "{asm}");
    assert!(!asm.contains(".16b"), "{asm}");
}
