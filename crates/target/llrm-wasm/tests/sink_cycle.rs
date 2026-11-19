use std::path::Path;
use std::process::Command;

/// An outer counter's `i + 1`, used only by the latch, stayed in the loop
/// header: it lived across the inner loop and cost a `local.get`/`local.set`
/// copy per trip (nbody -O2 +2400 instructions over the pin on the same IR).
#[test]
fn an_increment_used_only_by_the_latch_sinks_into_it() {
    let ir = Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/ir/sink_cycle.ll");
    let out = Command::new(env!("CARGO_BIN_EXE_llrm-wasm-llc"))
        .args(["-O2", "-S", "-o", "-"])
        .env("LLRM_WASM_BACKEND_ONLY", "1")
        .arg(ir)
        .output()
        .expect("llrm-wasm-llc");
    assert!(out.status.success());
    let asm = String::from_utf8_lossy(&out.stdout);
    let latch = asm.rsplit("end_block").next().unwrap();
    assert!(latch.contains("i32.const\t1"), "{asm}");
}
