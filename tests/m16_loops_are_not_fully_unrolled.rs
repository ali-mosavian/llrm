//! At m16 -O2 an 8-trip loop nest stays a loop: code bytes outweigh clocks.

mod common;

/// The pin's full-unroll threshold (150, boosted to 600) copied matmul's
/// 8-trip inner product 8 times inside 8 x 8 outer copies: 2045 code bytes at
/// -O2 against 306 rolled (main: 241), 29 `imul`s against 4.
#[test]
fn at_o2_a_nest_of_eight_trip_loops_keeps_its_multiplies_in_a_loop() {
    let scratch = tempfile::tempdir().unwrap();
    let directory = scratch.path();
    let source = format!("{}/bench/matmul/matmul.c", env!("CARGO_MANIFEST_DIR"));
    let output = common::cc::llrm_c()
        .current_dir(directory)
        .args(["-O2", "-m16", "-march=i386", "-S", "-o", "a.s", &source])
        .output()
        .unwrap();
    assert!(output.status.success(), "{}", String::from_utf8_lossy(&output.stderr));
    let text = std::fs::read_to_string(directory.join("a.s")).unwrap();
    let multiplies = text.matches("imul").count();
    assert!(multiplies <= 8, "{multiplies} multiplies\n{text}");
}
