//! A loop of two trips whose test folds on the trip (`i != 1`, `beta == 1.0`)
//! stayed a loop at -Os m16: the pin's `OptSizeThreshold` is 0, so the 13-unit
//! body was never "small enough" and float_in_place_update.c was 132 bytes
//! where the unrolled, folded main is 14 (DEVIATE U1,
//! `RealMode::unroll_size_few_trips`).

mod common;

#[test]
fn a_two_trip_loop_whose_tests_fold_is_unrolled_at_os_m16() {
    let source = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/run/c/float_in_place_update.c");
    let scratch = tempfile::tempdir().unwrap();
    let output = common::cc::llrm_c()
        .current_dir(scratch.path())
        .args(["-m16", "-Os", "-S", "-o", "a.s"])
        .arg(&source)
        .output()
        .unwrap();
    assert!(output.status.success(), "{}", String::from_utf8_lossy(&output.stderr));
    let asm = std::fs::read_to_string(scratch.path().join("a.s")).unwrap();
    let main: Vec<&str> = asm
        .lines()
        .skip_while(|l| !(l.contains("main") && l.contains(" proc")))
        .take_while(|l| !(l.contains("main") && l.contains(" endp")))
        .collect();
    let jumps = main.iter().filter(|l| l.trim_start().starts_with('j')).count();
    assert_eq!(jumps, 0, "the loop is left: {asm}");
}
