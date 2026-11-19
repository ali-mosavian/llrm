use std::path::Path;
use std::process::Command;

/// The block before a duplicated tail ended in a `COPY`, so the tail was never
/// merged into it: hanoi -O1 returned its constant through a local
/// (`i32.const 0; local.set 0; local.get 0; return`), 4 B over the pin.
#[test]
fn the_block_before_a_tail_takes_the_tail_when_it_ends_in_a_copy() {
    let ir = Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/ir/tail_dup_merge.ll");
    let out = Command::new(env!("CARGO_BIN_EXE_llrm-wasm-llc"))
        .args(["-S", "-O1", "-o", "-"])
        .arg(ir)
        .env("LLRM_WASM_BACKEND_ONLY", "1")
        .output()
        .expect("llrm-wasm-llc");
    assert!(out.status.success());
    let asm = String::from_utf8_lossy(&out.stdout).into_owned();
    let hanoi = asm.split("\nhanoi:").nth(1).unwrap().split("end_function").next().unwrap();
    assert!(!hanoi.contains("local.set"), "{hanoi}");
}
