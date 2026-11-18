//! `DYNAMIC_STACKALLOC`, `STACKSAVE` and `STACKRESTORE` on wasm: the shared
//! expansion over `SP32`, a frame pointer, and the write-back
//! (`WebAssemblyFrameLowering`, `WebAssemblyISelLowering.cpp` L373).

use std::path::Path;
use std::process::Command;

/// `tests/ir/dynamic_alloca.ll` through the backend alone, at `level`.
fn assembly(level: &str) -> String {
    let ir = Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/ir/dynamic_alloca.ll");
    let made = Command::new(env!("CARGO_BIN_EXE_llrm-wasm-llc"))
        .env("LLRM_WASM_BACKEND_ONLY", "1")
        .args([level, "-S", "-o", "-"])
        .arg(&ir)
        .output()
        .expect("llrm-wasm-llc");
    assert!(made.status.success(), "{level}: {}", String::from_utf8_lossy(&made.stderr));
    String::from_utf8(made.stdout).expect("utf8")
}

/// The body of `name`: its lines between the label and `end_function`.
fn body(
    asm: &str,
    name: &str,
) -> Vec<String> {
    let label = format!("{name}:");
    asm.lines()
        .skip_while(|line| !line.starts_with(&label))
        .skip(1)
        .take_while(|line| !line.contains("end_function"))
        .map(|line| line.trim().to_string())
        .filter(|line| !line.starts_with('.') && !line.starts_with('#'))
        .collect()
}

/// A function with an alloca made no code: `Legalize(DynamicStackAlloc,
/// "convert node to libcall")` refused `alloca_frame.c` and
/// `vla_callee_constant.c` while the pin compiles both.
#[test]
fn an_alloca_moves_the_stack_pointer_and_the_epilogue_puts_it_back() {
    for level in ["-O1", "-O2"] {
        let f = body(&assembly(level), "f").join("\n");
        let wanted = "global.get\t__stack_pointer\nlocal.tee\t1\nlocal.get\t0\ni32.const\t2\ni32.shl\ni32.const\t15\ni32.add\n\
                      i32.const\t-16\ni32.and\ni32.sub\nlocal.tee\t0\nglobal.set\t__stack_pointer\nlocal.get\t0\ncall\tuse\n\
                      local.get\t1\nglobal.set\t__stack_pointer";
        assert_eq!(f, wanted.replace("\\\n", ""), "{level}");
    }
}

/// `stacksave` read the stack pointer register itself, not a copy: the saved
/// value changed with every alloca, and `stackrestore` restored nothing (a
/// loop of them ran out of memory on node).
#[test]
fn a_saved_stack_pointer_is_a_copy_from_before_the_alloca() {
    for level in ["-O1", "-O2"] {
        let g = body(&assembly(level), "g");
        let first = g.iter().position(|line| line == "global.get\t__stack_pointer").expect("reads the stack pointer");
        assert_eq!(g[first + 1], "local.tee\t1", "{level}: {g:?}");
        let last = g.len();
        assert_eq!(g[last - 2..], ["local.get\t1", "global.set\t__stack_pointer"], "{level}: {g:?}");
    }
}

/// An alloca bracketed by save and restore in a loop compiles at every level
/// (the copy folds of `peephole_opt` and `machine_cse` rewrote the
/// stack pointer's uses to a register defined after them: "a unique value").
#[test]
fn an_alloca_in_a_loop_compiles_at_every_level() {
    for level in ["-O0", "-O1", "-O2", "-O3", "-Os"] {
        let h = body(&assembly(level), "h");
        assert!(h.iter().filter(|line| line.as_str() == "global.set\t__stack_pointer").count() >= 2, "{level}: {h:?}");
    }
}

/// The restore after the call copies the register the save went to: the
/// pin's `local.get 1 ; local.set 1`. `stacksave` that read the stack pointer
/// register itself restored nothing, and a loop of them leaked an alloca per
/// trip.
#[test]
fn a_restore_in_a_loop_writes_the_stack_pointer_register() {
    let h = body(&assembly("-O2"), "h");
    let call = h.iter().position(|line| line == "call\tuse").expect("the call");
    assert_eq!(h[call + 1..call + 3], ["local.get\t1", "local.set\t1"], "{h:?}");
}
