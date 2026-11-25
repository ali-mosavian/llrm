//! The pin's AArch64 clang and the tools that run its output, found one way
//! for every test that needs them (`llrm-arm64` kernels, `llrm-cc` inline asm).

use std::path::PathBuf;
use std::process::Command;

/// The pin clang built with AArch64, when it and the cross gcc and qemu are
/// all there.
pub fn clang() -> Option<PathBuf> {
    let home = std::env::var("HOME").unwrap_or_default();
    let build = std::env::var("LLVM_BUILD").unwrap_or_else(|_| format!("{home}/scratch/llvm-build-aarch64"));
    let clang = PathBuf::from(build).join("bin/clang");
    let tools =
        ["aarch64-linux-gnu-gcc", "qemu-aarch64"].iter().all(|t| Command::new(t).arg("--version").output().is_ok());
    (clang.exists() && tools).then_some(clang)
}
