//! A vector in the IR is refused, not a panic: the mid-end assumes scalars, and no target has vector MIR yet.

use std::process::Command;

/// `<4 x i32>` reached `combine_shared::width` and panicked ("an integer", exit 101, every llc but arm64 and wasm
/// untested); it is refused as isel refuses a type with no registers, exit 1.
#[test]
fn a_vector_value_is_refused_not_a_panic() {
    let dir = std::env::temp_dir().join("llrm-refuse-vector-x86_64");
    std::fs::create_dir_all(&dir).unwrap();
    let source = dir.join("g.ll");
    std::fs::write(
        &source,
        "target datalayout = \"e-m:e-p270:32:32-p271:32:32-p272:64:64-i64:64-i128:128-f80:128-n8:16:32:64-S128\"\ntarget triple = \"x86_64-unknown-linux-gnu\"\n\
         define <4 x i32> @g(<4 x i32> %a, <4 x i32> %b) {\n  %r = add <4 x i32> %a, %b\n  ret <4 x i32> %r\n}\n",
    )
    .unwrap();
    let done = Command::new(env!("CARGO_BIN_EXE_llrm-llc"))
        .args(["-S", "-o", dir.join("g.s").to_str().unwrap(), source.to_str().unwrap()])
        .output()
        .expect("the driver runs");
    let stderr = String::from_utf8_lossy(&done.stderr);
    assert_eq!(done.status.code(), Some(1), "{stderr}");
    assert!(stderr.contains("@g: a <4 x i32> value"), "{stderr}");
}

/// The same through the DOS targets of `llrm-llc` (`-m32`).
#[test]
fn a_vector_value_is_refused_not_a_panic_x86_32() {
    let dir = std::env::temp_dir().join("llrm-refuse-vector-x86_32");
    std::fs::create_dir_all(&dir).unwrap();
    let source = dir.join("g.ll");
    std::fs::write(
        &source,
        "target datalayout = \"e-p:32:32-i8:8-i16:16-i32:32-i64:32-f32:32-f64:32-f80:32-n8:16:32-S32\"\ntarget triple = \"i386-unknown-linux-gnu\"\n\
         define <4 x i32> @g(<4 x i32> %a, <4 x i32> %b) {\n  %r = add <4 x i32> %a, %b\n  ret <4 x i32> %r\n}\n",
    )
    .unwrap();
    let done = Command::new(env!("CARGO_BIN_EXE_llrm-llc"))
        .args(["-m32", "-S", "-o", dir.join("g.s").to_str().unwrap(), source.to_str().unwrap()])
        .output()
        .expect("the driver runs");
    let stderr = String::from_utf8_lossy(&done.stderr);
    assert_eq!(done.status.code(), Some(1), "{stderr}");
    assert!(stderr.contains("a <4 x i32> value"), "{stderr}");
}

/// The same through the DOS targets of `llrm-llc` (`-m16`).
#[test]
fn a_vector_value_is_refused_not_a_panic_x86_16() {
    let dir = std::env::temp_dir().join("llrm-refuse-vector-x86_16");
    std::fs::create_dir_all(&dir).unwrap();
    let source = dir.join("g.ll");
    std::fs::write(
        &source,
        "target datalayout = \"e-p:16:16-p1:32:16:16:16-p2:16:16-p3:32:16:16:32-p4:32:16:16:16-i32:16-i64:16-f80:16-n8:16:32-ni:2:3\"\ntarget triple = \"i386-unknown-unknown-code16\"\n\
         define <4 x i32> @g(<4 x i32> %a, <4 x i32> %b) {\n  %r = add <4 x i32> %a, %b\n  ret <4 x i32> %r\n}\n",
    )
    .unwrap();
    let done = Command::new(env!("CARGO_BIN_EXE_llrm-llc"))
        .args(["-m16", "-S", "-o", dir.join("g.s").to_str().unwrap(), source.to_str().unwrap()])
        .output()
        .expect("the driver runs");
    let stderr = String::from_utf8_lossy(&done.stderr);
    assert_eq!(done.status.code(), Some(1), "{stderr}");
    assert!(stderr.contains("a <4 x i32> value"), "{stderr}");
}
