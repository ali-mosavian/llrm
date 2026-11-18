//! DEVIATE VP9: `-fllrm-vect-very-cheap` vectorizes no loop that would end in a
//! scalar epilogue.

fn asm(flags: &[&str]) -> String {
    let dir = tempfile::tempdir().unwrap();
    let (input, output) = (dir.path().join("t.ll"), dir.path().join("t.s"));
    std::fs::write(&input, include_str!("data/very_cheap_sum.ll")).unwrap();
    let said = std::process::Command::new(env!("CARGO_BIN_EXE_llrm-arm64-llc"))
        .args(["-S", "-O2"])
        .args(flags)
        .arg("-o")
        .arg(&output)
        .arg(&input)
        .output()
        .unwrap();
    assert!(said.status.success(), "{}", String::from_utf8_lossy(&said.stderr));
    std::fs::read_to_string(&output).unwrap()
}

/// QuickrBasic a64 -O2 (table.c `tabl_moved`): the sum over `n` entries was
/// vectorized with its epilogues, 1216 B against gcc's 428.
#[test]
fn a_sum_of_an_unknown_count_is_scalar_under_the_very_cheap_model() {
    assert!(asm(&[]).contains("addv"), "premise: the pin's -O2 vectorizes the loop");
    assert!(!asm(&["-fllrm-vect-very-cheap"]).contains("addv"));
}
