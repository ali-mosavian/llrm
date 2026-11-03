use std::path::Path;
use std::process::Command;

fn function(
    level: &str,
    name: &str,
) -> String {
    let ir = Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/ir/call_order.ll");
    let out = Command::new(env!("CARGO_BIN_EXE_llrm-wasm-llc"))
        .args(["-S", level, "-o", "-"])
        .arg(ir)
        .output()
        .expect("llrm-wasm-llc");
    assert!(out.status.success());
    let asm = String::from_utf8_lossy(&out.stdout).into_owned();
    let from = asm.find(&format!("\n{name}:")).expect("function");
    asm[from..].split("end_function").next().unwrap().to_owned()
}

/// RegStackify moved a `call` down past a store: `CALL` is in no instruction
/// table, so `query` saw an instruction that touches nothing. `h` stored
/// before it called `g`.
#[test]
fn a_call_stays_before_a_later_store() {
    for level in ["-O1", "-O2"] {
        let asm = function(level, "h");
        let (call, store) = (asm.find("call\tg"), asm.find("i32.store"));
        assert!(call < store, "{level}: {asm}");
    }
}

/// The same bug put the epilogue's `global.set __stack_pointer` before a call
/// that reads the frame: a callee with a frame of its own overwrote it.
#[test]
fn a_call_that_reads_the_frame_comes_before_the_stack_pointer_restore() {
    for level in ["-O1", "-O2"] {
        let asm = function(level, "frame");
        let (call, restore) = (asm.find("call\ttake"), asm.rfind("global.set\t__stack_pointer"));
        assert!(call < restore, "{level}: {asm}");
    }
}
