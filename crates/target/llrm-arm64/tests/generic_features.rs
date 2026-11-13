//! `-mcpu=generic` (Linux) has none of apple-m1's optional features (RCPC, LSE, FullFP16, ...). The one table is
//! apple-m1's; the CPU answers `Subtarget->hasX()` at selection time (Darwin keeps them).

use std::process::Command;

const LINUX_DATALAYOUT: &str =
    "e-m:e-p270:32:32-p271:32:32-p272:64:64-i8:8:32-i16:16:32-i64:64-i128:128-n32:64-S128-Fn32";
const MAC_DATALAYOUT: &str = "e-m:o-p270:32:32-p271:32:32-p272:64:64-i64:64-i128:128-n32:64-S128-Fn32";

fn assembly(
    triple: &str,
    datalayout: &str,
    body: &str,
) -> String {
    let dir = tempfile::tempdir().unwrap();
    let (input, output) = (dir.path().join("t.ll"), dir.path().join("t.s"));
    std::fs::write(&input, format!("target triple = \"{triple}\"\ntarget datalayout = \"{datalayout}\"\n{body}")).unwrap();
    let said = Command::new(env!("CARGO_BIN_EXE_llrm-arm64-llc"))
        .args(["-S", "-O2", "-o"])
        .arg(&output)
        .arg(&input)
        .output()
        .unwrap();
    assert!(said.status.success(), "{}", String::from_utf8_lossy(&said.stderr));
    String::from_utf8(std::fs::read(&output).unwrap()).unwrap()
}

const LINUX: &str = "aarch64-unknown-linux-gnu";
const MAC: &str = "arm64-apple-macosx";

const ACQUIRE_LOAD: &str = "define i32 @f(ptr %p) {\n  %v = load atomic i32, ptr %p acquire, align 4\n  ret i32 %v\n}\n";

/// An acquire load selected `ldapr` (RCPC, ARMv8.3) on generic, which only has `ldar`.
#[test]
fn an_acquire_load_is_ldar_without_rcpc() {
    let text = assembly(LINUX, LINUX_DATALAYOUT, ACQUIRE_LOAD);
    assert!(text.contains("ldar"), "{text}");
    assert!(!text.contains("ldapr"), "{text}");
}

/// The premise: apple-m1 (Darwin) keeps the features.
#[test]
fn darwin_keeps_the_apple_m1_features() {
    let text = assembly(MAC, MAC_DATALAYOUT, ACQUIRE_LOAD);
    assert!(text.contains("ldapr"), "{text}");
}
