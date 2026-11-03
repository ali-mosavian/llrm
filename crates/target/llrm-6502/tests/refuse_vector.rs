//! A vector in the IR is refused, not a panic: the mid-end assumes scalars, and no target has vector MIR yet.

use std::process::Command;

/// `<4 x i32>` reached `combine_shared::width` and panicked ("an integer", exit 101, every llc but arm64 and wasm
/// untested); it is refused as isel refuses a type with no registers, exit 1.
#[test]
fn a_vector_value_is_refused_not_a_panic() {
    let dir = std::env::temp_dir().join("llrm-refuse-vector-6502");
    std::fs::create_dir_all(&dir).unwrap();
    let source = dir.join("g.ll");
    std::fs::write(
        &source,
        "target datalayout = \"e-p:16:8-i16:8-i32:8-i64:8-f32:8-f64:8-a:8-n8\"\ntarget triple = \"mos-unknown-none\"\n\
         define <4 x i32> @g(<4 x i32> %a, <4 x i32> %b) {\n  %r = add <4 x i32> %a, %b\n  ret <4 x i32> %r\n}\n",
    )
    .unwrap();
    let done = Command::new(env!("CARGO_BIN_EXE_llrm-6502-llc"))
        .args(["-S", "-o", dir.join("g.s").to_str().unwrap(), source.to_str().unwrap()])
        .output()
        .expect("the driver runs");
    let stderr = String::from_utf8_lossy(&done.stderr);
    assert_eq!(done.status.code(), Some(1), "{stderr}");
    assert!(stderr.contains("@g: a <4 x i32> value"), "{stderr}");
}
