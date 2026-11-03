use std::path::Path;
use std::process::Command;

fn assembly(function: &str) -> String {
    let ir = Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/ir/stack_coloring.ll");
    let out = Command::new(env!("CARGO_BIN_EXE_llrm-wasm-llc"))
        .args(["-S", "-O1", "-o", "-"])
        .arg(ir)
        .output()
        .expect("llrm-wasm-llc");
    assert!(out.status.success());
    let text = String::from_utf8_lossy(&out.stdout).into_owned();
    let start = text.find(&format!("{function}:")).expect("the function");
    text[start..].split(".Lfunc_end").next().unwrap().to_owned()
}

/// Two 256-byte allocas that are never live together each kept a frame
/// object, a 512-byte frame where the pin's is 256 (jpeg: 400 B to the pin's
/// 336, so every offset past 127 took a second LEB byte).
#[test]
fn allocas_never_live_together_share_a_frame_object() {
    let text = assembly("disjoint");
    assert!(text.contains("i32.const\t256"), "{text}");
    assert!(!text.contains("i32.const\t512"), "{text}");
}

/// `sp + frame offset + 8` was `i32.const off; i32.add; i32.const 8; i32.add`;
/// the pin folds the offset into the single-use constant (jpeg -O3 +247 B once
/// the sharing moved the hot object off offset 0).
#[test]
fn a_frame_offset_folds_into_the_constant_added_to_it() {
    let text = assembly("offset");
    assert!(text.contains("i32.const\t72\n"), "{text}");
    assert!(!text.contains("i32.const\t8\n"), "{text}");
}
