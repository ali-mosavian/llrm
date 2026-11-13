use std::path::Path;
use std::process::Command;

/// `zext nneg x` beside an existing `sext x` is that `sext`
/// (`visitZERO_EXTEND`'s closing arm, DAGCombiner.cpp#L16730). Without it the
/// load was `i32.load16_s` and the `zext nneg` a second extension, `i32.const
/// 65535; i32.and` (pattern_fill -O2, +3 B).
#[test]
fn a_zext_nneg_beside_its_sext_makes_no_mask() {
    let ir = Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/ir/zext_nneg_with_sext.ll");
    let out = Command::new(env!("CARGO_BIN_EXE_llrm-wasm-llc"))
        .args(["-O2", "-S", "-o", "-"])
        .arg(ir)
        .output()
        .expect("llrm-wasm-llc");
    assert!(out.status.success());
    let text = String::from_utf8_lossy(&out.stdout);
    assert!(text.contains("i32.load16_s"), "{text}");
    assert!(!text.contains("i32.and"), "{text}");
}
