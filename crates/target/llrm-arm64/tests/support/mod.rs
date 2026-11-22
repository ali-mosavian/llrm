//! Kernels built by this compiler from the pinned clang's un-optimized IR,
//! linked statically with the cross gcc and run under `qemu-aarch64`.

use std::path::{Path, PathBuf};
use std::process::Command;

fn clang() -> Option<PathBuf> {
    let home = std::env::var("HOME").unwrap_or_default();
    let build = std::env::var("LLVM_BUILD").unwrap_or_else(|_| format!("{home}/scratch/llvm-build-aarch64"));
    let clang = PathBuf::from(build).join("bin/clang");
    let tools =
        ["aarch64-linux-gnu-gcc", "qemu-aarch64"].iter().all(|t| Command::new(t).arg("--version").output().is_ok());
    (clang.exists() && tools).then_some(clang)
}

/// What `tests/kernels/<kernel>.c` prints when `llrm-arm64-llc` builds it at
/// `level` (`-O1`, `-O2`, `-Os`); `None` where the tools are missing.
pub fn output(
    kernel: &str,
    level: &str,
) -> Option<String> {
    let Some(clang) = clang() else {
        eprintln!("skipped: needs the pinned clang, aarch64-linux-gnu-gcc and qemu-aarch64");
        return None;
    };
    let source = Path::new(env!("CARGO_MANIFEST_DIR")).join(format!("tests/kernels/{kernel}.c"));
    let dir = tempfile::tempdir().unwrap();
    let (ir, object, program) = (dir.path().join("k.ll"), dir.path().join("k.o"), dir.path().join("k"));
    let run = |command: &mut Command| {
        let said = command.output().expect("the tool starts");
        assert!(said.status.success(), "{command:?}: {}", String::from_utf8_lossy(&said.stderr));
        String::from_utf8_lossy(&said.stdout).into_owned()
    };
    run(Command::new(&clang)
        .args(["-target", "aarch64-unknown-linux-gnu", "-O2", "-Xclang", "-disable-llvm-passes"])
        .args(["-Xclang", "-disable-lifetime-markers", "-S", "-emit-llvm", "-o"])
        .arg(&ir)
        .arg(&source));
    run(Command::new(env!("CARGO_BIN_EXE_llrm-arm64-llc"))
        .env("LLRM_VERIFY", "0")
        .arg(level)
        .arg("-o")
        .arg(&object)
        .arg(&ir));
    run(Command::new("aarch64-linux-gnu-gcc").arg("-static").arg("-o").arg(&program).arg(&object));
    Some(run(Command::new("qemu-aarch64").arg(&program)))
}
