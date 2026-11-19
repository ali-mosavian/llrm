use std::path::Path;
use std::process::Command;

/// A switch on an `i8` was left narrow, so its jump-table index was
/// `(x + 64) & 255` where the pin's CodeGenPrepare widens the switch to `i32`
/// and indexes with `x + -192` (87 bytes against the pin's 81 for five cases).
/// The mid-end ran none of CodeGenPrepare's lowering-gated steps because wasm
/// had no `Lowering` to ask.
#[test]
fn a_switch_on_a_byte_is_widened_and_its_index_is_not_masked() {
    let dir = std::env::temp_dir().join(format!("wasm-switch-widen-{}", std::process::id()));
    std::fs::create_dir_all(&dir).expect("a directory");
    let ir = Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/ir/switch_widen.ll");
    let object = dir.join("o.wasm");
    let built = Command::new(env!("CARGO_BIN_EXE_llrm-wasm-llc"))
        .args(["-O1", "-filetype=obj", "-o"])
        .arg(&object)
        .arg(ir)
        .output()
        .expect("llrm-wasm-llc");
    assert!(built.status.success(), "{}", String::from_utf8_lossy(&built.stderr));
    let dump = Command::new("wasm-objdump").arg("-d").arg(&object).output().expect("wasm-objdump");
    let _ = std::fs::remove_dir_all(&dir);
    let text = String::from_utf8(dump.stdout).expect("text");
    let ops: Vec<&str> = text.lines().filter_map(|line| line.split_once('|')).map(|(_, op)| op.trim()).collect();
    let table = ops.iter().position(|op| op.starts_with("br_table")).expect("a br_table");
    let load = ops[..table].iter().rposition(|op| op.starts_with("i32.load8_u")).expect("the byte load");
    let index = &ops[load..table];
    assert!(!index.iter().any(|op| *op == "i32.and"), "the index is masked: {index:?}");
}
