use std::path::Path;
use std::process::Command;

/// The DAG builder took x86's 16 stores for a memset, not the wasm default 8:
/// a 72-byte clear was nine `i64.store`s where the pin has one `memory.fill`
/// (`memory_expansion` -O2: +2579 B).
#[test]
fn a_memset_of_more_than_eight_stores_is_a_memory_fill() {
    let ir = Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/ir/memset_over_limit.ll");
    let out = Command::new(env!("CARGO_BIN_EXE_llrm-wasm-llc"))
        .args(["-S", "-O2", "-o", "-"])
        .arg(ir)
        .env("LLRM_WASM_BACKEND_ONLY", "1")
        .output()
        .expect("llrm-wasm-llc");
    assert!(out.status.success());
    let asm = String::from_utf8_lossy(&out.stdout);
    assert_eq!((asm.matches("memory.fill").count(), asm.matches("i64.store").count()), (1, 0), "{asm}");
}
