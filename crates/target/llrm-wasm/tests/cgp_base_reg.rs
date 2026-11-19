use std::path::Path;
use std::process::Command;

/// A scale-1 index of a global was an index, which wasm has none of, so
/// CodeGenPrepare left `@g + i + 8` made in the entry block and the other
/// block read it through a register (nbody_single -O3 +90 B, +136k
/// instructions over the pin).
#[test]
fn a_global_plus_index_plus_offset_is_made_where_it_is_used() {
    let ir = Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/ir/cgp_base_reg.ll");
    let out = Command::new(env!("CARGO_BIN_EXE_llrm-wasm-llc"))
        .args(["-O3", "-S", "-o", "-"])
        .env("LLRM_WASM_IR", "1")
        .arg(ir)
        .output()
        .expect("llrm-wasm-llc");
    assert!(out.status.success());
    let module = String::from_utf8_lossy(&out.stderr);
    let block = module.split("\nb:").nth(1).expect("block b").split("\n}").next().unwrap();
    assert!(block.contains("getelementptr i8, ptr @g"), "{module}");
}
