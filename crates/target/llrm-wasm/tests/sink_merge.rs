use std::path::Path;
use std::process::Command;

/// Two constants feeding one `ret` PHI sank over one critical edge only, as the
/// `CEMergeCandidates` key (the implicit def of `ARGUMENTS`) was left out; the
/// other stayed in the entry block, so the early return was a PHI copy through
/// a local, not `i32.const 1; return` (queens -O2 and sieve -O2 were 2 and 5
/// bytes under the pin, queens -O1 had one `return` to the pin's two).
#[test]
fn constants_held_back_for_one_block_split_both_edges() {
    let ir = Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/ir/sink_merge.ll");
    let out = Command::new(env!("CARGO_BIN_EXE_llrm-wasm-llc"))
        .args(["-S", "-O1", "-o", "-"])
        .arg(ir)
        .output()
        .expect("llrm-wasm-llc");
    assert!(out.status.success());
    let text = String::from_utf8_lossy(&out.stdout);
    let returns = text.lines().filter(|line| line.trim() == "return").count();
    assert_eq!(returns, 2, "{text}");
}
