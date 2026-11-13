use std::path::Path;
use std::process::Command;

/// `lftr_any_stride` took the i32 copy of `for (i = n - 1; i >= 0; --i)`'s
/// counter, wider than the i16 exit count, and compared it by `trunc` to i16:
/// two `i32.const 65535; i32.and` masks in the loop that the original test
/// never paid (wasm32 idioms +77 B at -O2, +74 B at -O3). Only the start may
/// mask, outside the loop.
#[test]
fn a_down_counter_wider_than_its_exit_count_is_not_masked_in_the_loop() {
    let ir = Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/ir/lftr_down_counter.ll");
    let out = Command::new(env!("CARGO_BIN_EXE_llrm-wasm-llc"))
        .args(["-S", "-Os", "-o", "-"])
        .arg(ir)
        .output()
        .expect("llrm-wasm-llc");
    assert!(out.status.success());
    let text = String::from_utf8_lossy(&out.stdout);
    let body = text.split("\tloop").nth(1).expect("a loop").split("\tend_loop").next().unwrap();
    let masks = body.lines().filter(|line| line.trim() == "i32.const\t65535").count();
    assert_eq!(masks, 0, "{masks} masks in the loop:\n{text}");
}
