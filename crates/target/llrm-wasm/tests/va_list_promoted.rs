use std::path::Path;
use std::process::Command;

/// wasm's module is single-threaded, so every object is thread-local and LICM
/// sinks the store of the `va_list` slot with its load (the pin's
/// `isThreadLocalObject`). The store stayed unsafe and the slot was read back
/// after `va_start`: long_double_variadic `sum` +12 B at -Os.
#[test]
fn the_va_list_slot_is_not_read_back_after_va_start() {
    for level in ["-O2", "-Os"] {
        let ir = Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/ir/va_list_loop.ll");
        let out = Command::new(env!("CARGO_BIN_EXE_llrm-wasm-llc"))
            .args(["-S", level, "-o", "-"])
            .arg(ir)
            .output()
            .expect("llrm-wasm-llc");
        assert!(out.status.success(), "{level}: {}", String::from_utf8_lossy(&out.stderr));
        let asm = String::from_utf8_lossy(&out.stdout).into_owned();
        assert!(!asm.contains("i32.load\t76"), "{level}: the slot is loaded in:\n{asm}");
    }
}
