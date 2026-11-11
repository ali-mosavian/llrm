use std::path::Path;
use std::process::Command;

/// The loop pass manager runs its whole list (instsimplify, simplifycfg, licm,
/// rotate, licm, unswitch) again on a loop `SimpleLoopUnswitch` asks to
/// revisit and on the loops it cloned (`revisitCurrentLoop`,
/// `addSiblingLoops`). Ours ran the unswitch alone, once: the clone kept the
/// `a = select(c, K, a)` loop the pin unswitches and deletes. The 14-line
/// reduction of known_loads_walks was 355 B at -O3 with five loops, the pin's
/// 254 B with three.
#[test]
fn an_unswitched_loop_is_simplified_and_unswitched_again_before_the_pass_moves_on() {
    let ir = Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/ir/unswitch_revisit.ll");
    let out = Command::new(env!("CARGO_BIN_EXE_llrm-wasm-llc"))
        .args(["-S", "-O3", "-o", "-"])
        .arg(ir)
        .output()
        .expect("llrm-wasm-llc");
    assert!(out.status.success());
    let text = String::from_utf8_lossy(&out.stdout);
    let loops = text.lines().filter(|line| line.trim() == "loop").count();
    assert_eq!(loops, 3, "{loops} loops:\n{text}");
}
