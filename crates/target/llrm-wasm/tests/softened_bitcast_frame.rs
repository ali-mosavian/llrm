use std::path::Path;
use std::process::Command;

/// A bitcast from a softened `fp128` to its two halves went through a stack
/// slot (`ExpandRes_BITCAST` of a float) where the pin splits the integer that
/// stands for the float. The combiner removed the stores and loads but not the
/// slot: a loop adding `fp128` had a 32-byte frame for one 16-byte result
/// (long_double_variadic -Os, frame 160 where the pin's is 112, +33 B).
#[test]
fn a_softened_fp128_is_split_without_a_stack_slot() {
    let ir = Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/ir/fp128_sum_in_loop.ll");
    let out = Command::new(env!("CARGO_BIN_EXE_llrm-wasm-llc"))
        .args(["-S", "-O2", "-o", "-"])
        .arg(ir)
        .output()
        .expect("llrm-wasm-llc");
    assert!(out.status.success());
    let asm = String::from_utf8_lossy(&out.stdout);
    assert!(asm.contains("i32.const\t16\n\ti32.sub"), "{asm}");
}
