use std::path::Path;
use std::process::Command;

/// Two loops counting to the same `zext nneg i16 %n`: the runtime unroll of the
/// second expanded its count in a `ScalarEvolution` of its own, so it did not
/// reuse the first's `add nsw %w, -1`. The compare then had one user, was
/// narrowed to i16, and LSR rebuilt the `zext` without `nneg`: `i32.const
/// 65535` + `i32.and` after the `i16` load (pattern_fill -O2, +6 B).
#[test]
fn two_loops_with_one_count_do_not_mask_it_to_16_bits() {
    let ir = Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/ir/unroll_shared_count.ll");
    let out = Command::new(env!("CARGO_BIN_EXE_llrm-wasm-llc"))
        .args(["-S", "-O2", "-o", "-"])
        .arg(ir)
        .output()
        .expect("llrm-wasm-llc");
    assert!(out.status.success());
    let asm = String::from_utf8_lossy(&out.stdout);
    assert!(!asm.contains("65535"), "{asm}");
}
