use std::path::Path;
use std::process::Command;

/// `tests/ir/countdown_exit_counter.ll` as assembly at -O2.
fn assembly() -> String {
    let ir = Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/ir/countdown_exit_counter.ll");
    let made = Command::new(env!("CARGO_BIN_EXE_llrm-wasm-llc"))
        .args(["-O2", "-S", "-o", "-"])
        .arg(&ir)
        .output()
        .expect("llrm-wasm-llc");
    assert!(made.status.success(), "{}", String::from_utf8_lossy(&made.stderr));
    String::from_utf8_lossy(&made.stdout).into_owned()
}

/// The unrolled loop's exit tested a counter scaled by 39 (`i32.const -312`,
/// `i32.const -15; i32.ne`) beside the plain one, where the pin tests the plain
/// one with a bare `br_if` (tail_calls -O2 +9 B): LSR's LG1 and LG2 scaled the
/// compare past the overflow check the pin keeps.
#[test]
fn the_exit_of_an_unrolled_loop_tests_the_plain_counter() {
    let text = assembly();
    assert!(!text.contains("-312"), "{text}");
}
