//! `long double` is binary128 on arm64 Linux: operations are `tf` calls with
//! `q` register arguments. Each kernel is built by this compiler from the
//! pinned clang's un-optimized IR, linked statically with the cross gcc (its
//! libgcc has the `tf` routines) and run under `qemu-aarch64`.

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

fn output(kernel: &str) -> Option<String> {
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
    run(Command::new(env!("CARGO_BIN_EXE_llrm-arm64-llc")).env("LLRM_VERIFY", "0").arg("-o").arg(&object).arg(&ir));
    run(Command::new("aarch64-linux-gnu-gcc").arg("-static").arg("-o").arg(&program).arg(&object));
    Some(run(Command::new("qemu-aarch64").arg(&program)))
}

/// Register-to-register moves of an `f128` were `fmov d`: 64 of its 128 bits.
/// The kernel printed `0 0` for `28 773`.
#[test]
fn the_long_double_kernel_prints_what_gcc_prints() {
    if let Some(said) = output("long_double") {
        assert_eq!(said, "28 773\n");
    }
}

/// `sitofp i32 7 to fp128` folded to a double's bits: `mix(7, x, 2, y, 100)`
/// printed 6 for 111.
#[test]
fn integers_converted_to_long_double_keep_their_value() {
    if let Some(said) = output("long_double_mix") {
        assert_eq!(said, "111\n");
    }
}
