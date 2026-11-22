use std::path::Path;
use std::process::Command;

/// vla_loop_stack_restored wasm -O3 printed another answer than the pin: the
/// loop calls `vla`, an internal function with one use, which the pin counts
/// an inline candidate (`CodeMetrics`, `Function::hasOneLiveUse`) and leaves
/// the loop rolled. Loop unrolling looked for the uses in declarations that
/// hold no body, found none and unrolled it: six trips of `stacksave; alloca;
/// stackrestore` became six frame slots, each lower than the last.
#[test]
fn a_loop_calling_an_internal_function_called_once_is_not_unrolled() {
    let ir = Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/ir/vla_internal_callee_loop.ll");
    let out = Command::new(env!("CARGO_BIN_EXE_llrm-wasm-llc"))
        .args(["-S", "-O3", "-o", "-"])
        .arg(ir)
        .output()
        .expect("llrm-wasm-llc");
    assert!(out.status.success(), "{}", String::from_utf8_lossy(&out.stderr));
    let asm = String::from_utf8_lossy(&out.stdout);
    let calls = asm.lines().filter(|line| line.trim() == "call\tvla").count();
    assert!(calls <= 1, "{calls} calls of vla, the loop was unrolled:\n{asm}");
}
