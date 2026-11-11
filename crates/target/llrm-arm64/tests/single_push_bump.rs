//! A frame whose first push is one register keeps the pre-index range of a
//! single store (-256), not the pair's (-512).

use std::process::Command;

/// `store_merge_interleaved.ll` saved one register at O1 with `str x30, [sp, #-272]!`, which
/// does not encode: the object failed with "a pre-index offset -272 does not fit 9 bits".
#[test]
fn a_single_register_first_push_does_not_take_the_pairs_range() {
    for level in ["-O1", "-O2", "-Os"] {
        let scratch = tempfile::tempdir().unwrap();
        let out = scratch.path().join("f.o");
        let done = Command::new(env!("CARGO_BIN_EXE_llrm-arm64-llc"))
            .args([level, "-o", out.to_str().unwrap(), concat!(env!("CARGO_MANIFEST_DIR"), "/tests/data/store_merge_interleaved.ll")])
            .output()
            .expect("the driver runs");
        assert!(done.status.success(), "{level}: {}", String::from_utf8_lossy(&done.stderr));
        assert!(out.exists(), "{level}: no object");
    }
}

/// A 232-byte local and a call: the frame folds to 256 bytes, and `ldr x30, [sp], #256` (post-index
/// stops at 255) failed to encode.
#[test]
fn a_frame_of_256_bytes_is_not_folded_into_a_single_load() {
    let scratch = tempfile::tempdir().unwrap();
    let dir = scratch.path();
    let (source, out) = (dir.join("f.ll"), dir.join("f.o"));
    let _ = std::fs::remove_file(&out);
    std::fs::write(
        &source,
        "target datalayout = \"e-m:e-p270:32:32-p271:32:32-p272:64:64-i8:8:32-i16:16:32-i64:64-i128:128-n32:64-S128-Fn32\"\n\
target triple = \"aarch64-unknown-linux-gnu\"\ndeclare void @g(ptr)\n\
define void @f() {\n  %a = alloca [232 x i8], align 8\n  call void @g(ptr %a)\n  ret void\n}\n",
    )
    .unwrap();
    let done = Command::new(env!("CARGO_BIN_EXE_llrm-arm64-llc"))
        .args(["-O1", "-o", out.to_str().unwrap(), source.to_str().unwrap()])
        .output()
        .expect("the driver runs");
    assert!(done.status.success(), "{}", String::from_utf8_lossy(&done.stderr));
}
