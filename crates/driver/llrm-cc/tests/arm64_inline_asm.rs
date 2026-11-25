//! arm64 inline asm (`udiv`/`msub` with an early-clobber output, `rev ${0:w}`,
//! `fsqrt ${0:d}`, a memory output, a fixed `w0`) was refused by the backend
//! ("expected a constant, found asm"). A block is now one `llrm.a64.asm.*` call
//! whose words are laid down at the site. The gate needs the pin's AArch64
//! clang (`LLRM_A64_CLANG`, default `~/scratch/llvm-build-aarch64`), the
//! AArch64 binutils objcopy, `llrm-arm64-llc` built beside the driver,
//! `aarch64-linux-gnu-gcc` and `qemu-aarch64`: a missing tool fails the test.

use std::path::Path;
use std::process::Command;

fn tool_present(program: &str) -> bool {
    Command::new(program).arg("--version").output().is_ok()
}

#[test]
fn test_arm64_asm_blocks_compute_what_the_source_says_at_every_level() {
    for tool in ["aarch64-linux-gnu-gcc", "aarch64-linux-gnu-objcopy", "qemu-aarch64"] {
        assert!(tool_present(tool), "this test needs {tool}");
    }
    let source = Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/c/a64_inline_asm.c");
    for level in ["-O1", "-O2", "-Os"] {
        let dir = Path::new(env!("CARGO_TARGET_TMPDIR")).join(format!("a64asm-{level}-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        let object = dir.join("a.o");
        let done = Command::new(env!("CARGO_BIN_EXE_llrm-cc-bin"))
            .env("LLRM_CC_TARGET", "arm64-linux")
            .args([level, "-w", "-c", "-o"])
            .arg(&object)
            .arg(&source)
            .output()
            .unwrap();
        assert!(done.status.success(), "{level}: {}", String::from_utf8_lossy(&done.stderr));
        let program = dir.join("a.out");
        let linked =
            Command::new("aarch64-linux-gnu-gcc").arg("-static").arg("-o").arg(&program).arg(&object).output().unwrap();
        assert!(linked.status.success(), "{level}: {}", String::from_utf8_lossy(&linked.stderr));
        let ran = Command::new("qemu-aarch64").arg(&program).output().unwrap();
        assert_eq!(String::from_utf8_lossy(&ran.stdout), "14 2 44332211 1.50 77 42\n", "{level}");
    }
}
