use std::path::Path;
use std::process::Command;

/// vla_callee_constant wasm -O3 kept seven empty `stacksave`/`stackrestore`
/// groups after its arrays folded away (+6 instructions, +18 B over the pin).
/// The pin removes a restore that follows its save, and the dead save.
#[test]
fn empty_stack_save_and_restore_pairs_cost_nothing() {
    let ir = Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/ir/empty_stack_pairs.ll");
    let out = Command::new(env!("CARGO_BIN_EXE_llrm-wasm-llc"))
        .args(["-S", "-O2", "-o", "-"])
        .arg(ir)
        .output()
        .expect("llrm-wasm-llc");
    assert!(out.status.success(), "{}", String::from_utf8_lossy(&out.stderr));
    let asm = String::from_utf8_lossy(&out.stdout);
    assert!(!asm.contains("__stack_pointer"), "{asm}");
}
