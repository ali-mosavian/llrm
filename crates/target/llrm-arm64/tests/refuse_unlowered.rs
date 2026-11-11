//! An over-aligned object is a named refusal, not a misaligned one.

use std::process::Command;

const HEAD: &str = "target datalayout = \"e-m:e-p270:32:32-p271:32:32-p272:64:64-i8:8:32-i16:16:32-i64:64-i128:128-n32:64-S128-Fn32\"\ntarget triple = \"aarch64-unknown-linux-gnu\"\n";

/// The driver's stderr; the compile must exit
/// 1 and write no object.
fn refusal(
    tag: &str,
    body: &str,
) -> String {
    let scratch = tempfile::tempdir().unwrap();
    let dir = scratch.path();
    let (source, out) = (dir.join("g.ll"), dir.join("g.s"));
    let _ = std::fs::remove_file(&out);
    std::fs::write(&source, format!("{HEAD}{body}")).unwrap();
    let done = Command::new(env!("CARGO_BIN_EXE_llrm-arm64-llc"))
        .args(["-S", "-O2", "-o", out.to_str().unwrap(), source.to_str().unwrap()])
        .output()
        .expect("the driver runs");
    let stderr = String::from_utf8_lossy(&done.stderr).into_owned();
    assert_eq!(done.status.code(), Some(1), "{stderr}");
    assert!(!out.exists(), "wrote {}", out.display());
    stderr
}

/// `alloca [64 x i8], align 64` got `add x0, sp, #16` (sp is 16 aligned); the
/// mid-end also folds `ptr & 63` to zero on the promised alignment.
#[test]
fn an_object_aligned_past_the_stack_is_refused() {
    let why = refusal(
        "static",
        "define i64 @f() \"frame-pointer\"=\"all\" {\n  %a = alloca [64 x i8], align 64\n  store i8 1, ptr %a\n  %i = ptrtoint ptr %a to i64\n  ret i64 %i\n}\n",
    );
    assert!(why.contains("aligned to 64 needs stack realignment"), "{why}");
}

#[test]
fn a_dynamic_object_aligned_past_the_stack_is_refused() {
    let why = refusal(
        "dynamic",
        "define i64 @f(i64 %n) \"frame-pointer\"=\"all\" {\n  %a = alloca i8, i64 %n, align 64\n  store i8 1, ptr %a\n  %i = ptrtoint ptr %a to i64\n  ret i64 %i\n}\n",
    );
    assert!(why.contains("aligned to 64 needs stack realignment"), "{why}");
}
