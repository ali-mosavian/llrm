//! A `gep` read as a stored value and as a load address in another block was
//! kept as one shared address (`local.tee` then `i32.load 0`), where the pin's
//! CodeGenPrepare folds the offset into the load whenever the base register is
//! already read in that block (`isProfitableToFoldIntoAddressingMode`, the
//! `valueAlreadyLiveAtInst` test): a variadic walk was 4 instructions and 6 B
//! over the pin.

use std::path::Path;
use std::process::Command;

fn function(name: &str) -> String {
    let ir = Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/ir/address_fold_live_base.ll");
    let out = Command::new(env!("CARGO_BIN_EXE_llrm-wasm-llc"))
        .args(["-S", "-O2", "-o", "-"])
        .arg(ir)
        .output()
        .expect("llrm-wasm-llc");
    assert!(out.status.success(), "{}", String::from_utf8_lossy(&out.stderr));
    let text = String::from_utf8_lossy(&out.stdout).into_owned();
    let start = text.find(&format!("\n{name}:")).expect("function present");
    let end = text[start..].find("end_function").map_or(text.len(), |at| start + at);
    text[start..end].to_owned()
}

#[test]
fn an_offset_folds_into_a_load_where_the_base_is_already_read() {
    let asm = function("live");
    assert!(asm.contains("i32.load\t8"), "the offset was not folded:\n{asm}");
}

#[test]
fn a_shared_address_stays_where_folding_would_extend_the_base() {
    let asm = function("dead");
    assert!(asm.contains("i32.load\t0"), "the base was kept alive for a fold:\n{asm}");
}
