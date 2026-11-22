use std::path::Path;
use std::process::Command;

/// `select c2, true, c` (an `or`) where `c` is also the condition of another select: the pin's `visitFREEZE` turns
/// every use of the frozen value into the freeze, then pushes it down to the call results, so no freeze is left on `c`.
/// Ours froze `c` itself and `and (freeze c), 1` kept a third `i32.and` in loop_epilogue_sum's loop (+3 B, +302 instrs
/// at -Os and -O1); the pin has two.
#[test]
fn a_condition_used_by_a_select_and_an_or_keeps_no_freeze() {
    let ir = Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/ir/freeze_hoisted_over_shared_condition.ll");
    let out = Command::new(env!("CARGO_BIN_EXE_llrm-wasm-llc")).args(["-S", "-Os", "-o", "-"]).arg(ir).output().expect("llrm-wasm-llc");
    assert!(out.status.success());
    let asm = String::from_utf8_lossy(&out.stdout).into_owned();
    let ands = asm.lines().filter(|line| line.trim() == "i32.and").count();
    assert_eq!(ands, 2, "{asm}");
}
