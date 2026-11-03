//! A vector in the IR is refused, not a panic: the mid-end assumes scalars, and no target has vector MIR yet.

use std::process::Command;

/// `<4 x i32>` reached `combine_shared::width` and panicked ("an integer", exit 101, every llc but arm64 and wasm
/// untested); it is refused as isel refuses a type with no registers, exit 1.
#[test]
fn a_vector_value_is_refused_not_a_panic() {
    let dir = std::env::temp_dir().join("llrm-refuse-vector-m68k");
    std::fs::create_dir_all(&dir).unwrap();
    let source = dir.join("g.ll");
    std::fs::write(
        &source,
        "target datalayout = \"E-m:e-p:32:16:32-i8:8:8-i16:16:16-i32:16:32-n8:16:32-a:0:16-S16\"\ntarget triple = \"m68k-unknown-linux-gnu\"\n\
         define <4 x i32> @g(<4 x i32> %a, <4 x i32> %b) {\n  %r = add <4 x i32> %a, %b\n  ret <4 x i32> %r\n}\n",
    )
    .unwrap();
    let done = Command::new(env!("CARGO_BIN_EXE_llrm-m68k-llc"))
        .args(["-S", "-o", dir.join("g.s").to_str().unwrap(), source.to_str().unwrap()])
        .output()
        .expect("the driver runs");
    let stderr = String::from_utf8_lossy(&done.stderr);
    assert_eq!(done.status.code(), Some(1), "{stderr}");
    assert!(stderr.contains("@g: a <4 x i32> value"), "{stderr}");
}
